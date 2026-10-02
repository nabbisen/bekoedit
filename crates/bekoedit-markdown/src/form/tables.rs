//! Table block resolution for Form Mode (RFC-027, extended by RFC-048).
//!
//! Task 045: a cell edit changes only that cell's own content range. The
//! table is never regenerated -- every other byte (the separator line's
//! alignment markers, every other cell, this cell's own surrounding
//! whitespace, line endings) is left exactly as the user wrote it.
//!
//! Review (2026-10-02): cell boundaries come from `crate::gfm`'s
//! `table_cell_ranges`, straight from `pulldown-cmark`'s own `TableCell`
//! event ranges -- not from a second, hand-written splitter
//! cross-checked against it. Two splitters is how the first version of
//! this module still lost data: `form.rs`'s own display projection kept
//! the old naive "split on every `|`" parser, so a user editing the cell
//! the *display* showed them (miscounted, for a cell holding an escaped
//! pipe) sent a correct edit to the wrong byte range. `gfm::table_cell_ranges`
//! now serves the projection, this module's edit path, and
//! (RFC-048 slice 2) `index::blocks::classify_table`'s own decision of
//! whether a table is `SimpleTable` at all -- one function, so none of
//! the three can disagree with each other by construction. It also does
//! not fail for a cell with inline formatting, a link, an image, an
//! autolink or an entity: a boundary-based function does not care what
//! is inside a cell, only where it starts and ends.

use crate::block::{BlockKind, BlockNode};
use crate::form::{FormEditError, InlineFormat};
use crate::gfm::table_cell_ranges;
use crate::patch::PatchOrigin;
use crate::range::{ByteRange, utf16_to_utf8_offset};
use crate::trivia::LineEnding;

type Resolved = (ByteRange, String, PatchOrigin);

/// `|` to `\|` -- the text the user typed, as it is written back into the
/// source (task 045 §2.2's third bullet) -- the inverse of
/// `crate::gfm::unescape_pipe`.
fn escape_pipe(cell_text: &str) -> String {
    cell_text.replace('|', "\\|")
}

/// The byte range, absolute within `text`, of cell `(row, col)` -- the one
/// place [`resolve_replace_table_cell`] and
/// [`resolve_toggle_inline_in_table_cell`] both find a cell, so a toolbar
/// toggle and a direct cell edit can never disagree about where a cell is.
fn find_cell_range(
    text: &str,
    block: &BlockNode,
    row: usize,
    col: usize,
) -> Result<std::ops::Range<usize>, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let rows =
        table_cell_ranges(source).ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "table did not parse as a single table block".into(),
        })?;
    let row_cells = rows.get(row).ok_or(FormEditError::ItemNotFound {
        ordinal: row as u32,
    })?;
    let cell_range = row_cells.get(col).ok_or(FormEditError::ItemNotFound {
        ordinal: col as u32,
    })?;
    let abs_start = block.source_range.start + cell_range.start;
    let abs_end = block.source_range.start + cell_range.end;
    Ok(abs_start..abs_end)
}

pub fn resolve_replace_table_cell(
    text: &str,
    block: &BlockNode,
    row: usize,
    col: usize,
    cell_text: &str,
) -> Result<Resolved, FormEditError> {
    if cell_text.contains('\n') || cell_text.contains('\r') {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "a table cell's text cannot contain a newline".into(),
        });
    }
    let range = find_cell_range(text, block, row, col)?;
    Ok((
        ByteRange::new(range.start, range.end),
        escape_pipe(cell_text),
        PatchOrigin::FormMode,
    ))
}

/// Toggles inline markup around a UTF-16-offset selection within one
/// table cell (RFC-048 slice 2 §2.3, extended by task 048 D2). The
/// offsets are relative to `current_text` -- the cell's own live input
/// value, sent alongside the selection, not whatever the document
/// currently has stored (the same reason [`super::inline_fmt::resolve_toggle_inline`]
/// takes `current_text` for a block's own content). Computes the new cell
/// text from `current_text`, then reuses [`resolve_replace_table_cell`]
/// for the boundary and the re-escaping, so a toggle can never patch
/// outside the cell `ReplaceTableCell` itself would target, and always
/// commits the cell's pending text and the toggle as one patch.
pub fn resolve_toggle_inline_in_table_cell(
    text: &str,
    block: &BlockNode,
    cell: (usize, usize),
    current_text: &str,
    kind: InlineFormat,
    selection: (usize, usize),
    link_url: Option<&str>,
) -> Result<Resolved, FormEditError> {
    let (row, col) = cell;
    let (utf16_start, utf16_len) = selection;
    let byte_start = utf16_to_utf8_offset(current_text, utf16_start).ok_or_else(|| {
        FormEditError::InvalidEditPayload {
            reason: "invalid UTF-16 start offset".into(),
        }
    })?;
    let byte_end =
        utf16_to_utf8_offset(current_text, utf16_start + utf16_len).ok_or_else(|| {
            FormEditError::InvalidEditPayload {
                reason: "invalid UTF-16 end offset".into(),
            }
        })?;

    let selected = &current_text[byte_start..byte_end];
    let replaced = super::inline_fmt::toggled_text(selected, kind, link_url);
    let new_cell_text = format!(
        "{}{replaced}{}",
        &current_text[..byte_start],
        &current_text[byte_end..]
    );

    resolve_replace_table_cell(text, block, row, col, &new_cell_text)
}

pub fn resolve_add_table_row(text: &str, block: &BlockNode) -> Result<Resolved, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let rows =
        table_cell_ranges(source).ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "table did not parse as a single table block".into(),
        })?;
    let col_count = rows.first().map(Vec::len).unwrap_or(0);
    let le = LineEnding::detect(text).as_str();
    let new_row = format!("|{}", "  |".repeat(col_count));
    let insert_at = block.source_range.end;
    Ok((
        ByteRange::new(insert_at, insert_at),
        format!("{le}{new_row}"),
        PatchOrigin::FormMode,
    ))
}
