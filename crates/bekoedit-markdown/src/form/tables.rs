//! Table block resolution for Form Mode (RFC-027).
//!
//! Task 045: a cell edit changes only that cell's own content range. The
//! table is never regenerated -- every other byte (the separator line's
//! alignment markers, every other cell, this cell's own surrounding
//! whitespace, line endings) is left exactly as the user wrote it.
//!
//! Review (2026-10-02): cell boundaries come directly from
//! `pulldown-cmark`'s own `TableCell` event ranges, trimmed of
//! whitespace -- not from a second, hand-written splitter cross-checked
//! against it. Two splitters is how the first version of this module
//! still lost data: `form.rs`'s own display projection kept the old
//! naive "split on every `|`" parser, so a user editing the cell the
//! *display* showed them (miscounted, for a cell holding an escaped
//! pipe) sent a correct edit to the wrong byte range. One function,
//! [`table_cell_ranges`], now serves both the projection and the edit
//! path, so they cannot disagree with each other by construction. It
//! also does not fail for a cell with an inline link, image, autolink or
//! entity -- `classify_table` does not exclude any of those from
//! `SimpleTable`, and a boundary-based function does not care what is
//! inside a cell, only where it starts and ends.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::block::{BlockKind, BlockNode};
use crate::form::FormEditError;
use crate::index::parse_options;
use crate::patch::PatchOrigin;
use crate::range::ByteRange;
use crate::trivia::LineEnding;

type Resolved = (ByteRange, String, PatchOrigin);

/// Each row's cell byte ranges within `source` (one `SimpleTable` block's
/// own text), the header first, then each data row, in source order --
/// trimmed of each cell's own leading and trailing whitespace. `None` if
/// `source` does not parse as a single well-formed table (should not
/// happen for a block `classify_table` already accepted, since this
/// walks the same parser again over the same text; kept as a defensive
/// refusal, not a guess, if it ever does).
///
/// The one place both the edit path ([`resolve_replace_table_cell`],
/// [`resolve_add_table_row`]) and `form.rs`'s own projection get a
/// table's cell boundaries -- see the module doc comment for why that
/// matters.
pub fn table_cell_ranges(source: &str) -> Option<Vec<Vec<Range<usize>>>> {
    let mut rows: Vec<Vec<Range<usize>>> = Vec::new();
    let mut current_row: Option<Vec<Range<usize>>> = None;
    for (event, range) in Parser::new_ext(source, parse_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                current_row = Some(Vec::new());
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                rows.push(current_row.take()?);
            }
            Event::End(TagEnd::TableCell) => {
                current_row.as_mut()?.push(trim_cell_range(source, range));
            }
            _ => {}
        }
    }
    if rows.is_empty() { None } else { Some(rows) }
}

fn trim_cell_range(source: &str, range: Range<usize>) -> Range<usize> {
    let bytes = source.as_bytes();
    let mut start = range.start;
    let mut end = range.end;
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    start..end
}

/// `\|` to `|`, and nothing else unescaped (task 045 §2.2's Projection
/// rule, which the review confirmed also governs `form.rs`'s display
/// projection, not only the edit path) -- the inverse of
/// [`escape_pipe`].
pub fn unescape_pipe(cell_source: &str) -> String {
    let mut out = String::with_capacity(cell_source.len());
    let mut chars = cell_source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            out.push('|');
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// `|` to `\|` -- the text the user typed, as it is written back into the
/// source (task 045 §2.2's third bullet) -- the inverse of
/// [`unescape_pipe`].
fn escape_pipe(cell_text: &str) -> String {
    cell_text.replace('|', "\\|")
}

pub fn resolve_replace_table_cell(
    text: &str,
    block: &BlockNode,
    row: usize,
    col: usize,
    cell_text: &str,
) -> Result<Resolved, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    if cell_text.contains('\n') || cell_text.contains('\r') {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "a table cell's text cannot contain a newline".into(),
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
    Ok((
        ByteRange::new(abs_start, abs_end),
        escape_pipe(cell_text),
        PatchOrigin::FormMode,
    ))
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
