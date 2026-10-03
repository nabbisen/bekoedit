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

use std::ops::Range;

use crate::block::{BlockKind, BlockNode};
use crate::form::{FormEditError, InlineFormat, TableRowDirection, TableRowPosition};
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
    let new_row = empty_row_text(col_count);
    let insert_at = block.source_range.end;
    Ok((
        ByteRange::new(insert_at, insert_at),
        format!("{le}{new_row}"),
        PatchOrigin::FormMode,
    ))
}

// ─── Row operations (RFC-048 slice 3) ──────────────────────────────────────
//
// Each operation below moves or removes whole *lines*, never rewriting a
// row's own content: the line boundaries come from the same
// `table_cell_ranges` the rest of this module uses (scanning outward from
// a row's first/last cell to that physical line's own start and end), so
// they can never disagree with the parser about which bytes belong to
// which row. Every result is re-parsed (`verify_row_operation`) before it
// is returned: if the hypothetical output does not parse back to the
// expected shape, with every untouched cell exactly as it was, the whole
// operation is refused, never applied.

/// As many empty cells as `col_count`, in the table's own
/// leading/trailing-pipe style -- shared by [`resolve_add_table_row`] and
/// [`resolve_insert_table_row`], so a new row is always shaped the same
/// way regardless of which command produced it.
fn empty_row_text(col_count: usize) -> String {
    format!("|{}", "  |".repeat(col_count))
}

/// Absolute (document-wide, not block-relative) cell ranges for every row
/// of the table at `block` -- the same boundaries [`find_cell_range`]
/// resolves a single cell against, but for every row at once.
fn absolute_rows(text: &str, block: &BlockNode) -> Option<Vec<Vec<Range<usize>>>> {
    let source = &text[block.source_range.start..block.source_range.end];
    let rows = table_cell_ranges(source)?;
    Some(
        rows.into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|r| {
                        (block.source_range.start + r.start)..(block.source_range.start + r.end)
                    })
                    .collect()
            })
            .collect(),
    )
}

/// The absolute start of the physical line containing byte position `pos`.
fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map_or(0, |i| i + 1)
}

/// The absolute end of the physical line containing `pos`: the position
/// just before its own line ending (`\r\n` or `\n`), or `text.len()` if
/// that line is the file's last and has none.
fn line_content_end(text: &str, pos: usize) -> usize {
    match text[pos..].find('\n') {
        Some(rel) => {
            let nl = pos + rel;
            if nl > pos && text.as_bytes()[nl - 1] == b'\r' {
                nl - 1
            } else {
                nl
            }
        }
        None => text.len(),
    }
}

/// The line ending right after `content_end` (as [`line_content_end`]
/// returns it): empty (`content_end..content_end`) if there is none.
fn line_ending_after(text: &str, content_end: usize) -> Range<usize> {
    let bytes = text.as_bytes();
    if bytes.get(content_end) == Some(&b'\r') && bytes.get(content_end + 1) == Some(&b'\n') {
        content_end..content_end + 2
    } else if bytes.get(content_end) == Some(&b'\n') {
        content_end..content_end + 1
    } else {
        content_end..content_end
    }
}

/// The line ending right before `start` (as [`line_start`] returns it):
/// empty only if `start == 0`.
fn line_ending_before(text: &str, start: usize) -> Range<usize> {
    let bytes = text.as_bytes();
    if start >= 2 && bytes[start - 2] == b'\r' && bytes[start - 1] == b'\n' {
        (start - 2)..start
    } else if start >= 1 && bytes[start - 1] == b'\n' {
        (start - 1)..start
    } else {
        start..start
    }
}

/// Row `row`'s own full line span `(line_start, content_end)`, from
/// `rows`' cell ranges -- scanning outward from its first and last cell
/// to the physical line's own boundaries, so a leading/trailing pipe or
/// padding outside any cell's trimmed range is still part of the span.
fn row_line_span(text: &str, rows: &[Vec<Range<usize>>], row: usize) -> Option<(usize, usize)> {
    let cells = rows.get(row)?;
    let first = cells.first()?;
    let last = cells.last()?;
    Some((
        line_start(text, first.start),
        line_content_end(text, last.end),
    ))
}

fn cell_texts(text: &str, cells: &[Range<usize>]) -> Vec<String> {
    cells
        .iter()
        .map(|r| crate::gfm::unescape_pipe(&text[r.clone()]))
        .collect()
}

fn splice(text: &str, range: &Range<usize>, replacement: &str) -> String {
    format!(
        "{}{}{}",
        &text[..range.start],
        replacement,
        &text[range.end..]
    )
}

/// Re-parses the table's own hypothetical result after splicing
/// `range`/`replacement` into `text`: it must still parse as one table,
/// with exactly `expected_row_count` rows, and every `(new_row_index,
/// original_cell_texts)` pair in `unchanged` must still hold those exact
/// texts at that row. Refuses, rather than ever returning a patch whose
/// own result it has not itself checked.
fn verify_row_operation(
    text: &str,
    block: &BlockNode,
    range: &Range<usize>,
    replacement: &str,
    expected_row_count: usize,
    unchanged: &[(usize, Vec<String>)],
) -> Result<(), FormEditError> {
    let refuse = |reason: &str| FormEditError::UnsupportedEditOperation {
        reason: reason.to_string(),
    };
    let out = splice(text, range, replacement);
    let delta = replacement.len() as i64 - (range.end - range.start) as i64;
    let new_block_end = (block.source_range.end as i64 + delta) as usize;
    let new_source = &out[block.source_range.start..new_block_end];
    let rows = table_cell_ranges(new_source)
        .ok_or_else(|| refuse("row operation result did not re-parse as a table"))?;
    if rows.len() != expected_row_count {
        return Err(refuse("row operation result has an unexpected row count"));
    }
    for (row_idx, expected_cells) in unchanged {
        let row = rows
            .get(*row_idx)
            .ok_or_else(|| refuse("row operation result is missing an unchanged row"))?;
        if row.len() != expected_cells.len() {
            return Err(refuse(
                "row operation result changed an unchanged row's column count",
            ));
        }
        for (col, expected) in expected_cells.iter().enumerate() {
            let actual = crate::gfm::unescape_pipe(&new_source[row[col].clone()]);
            if &actual != expected {
                return Err(refuse("row operation result changed an untouched cell"));
            }
        }
    }
    Ok(())
}

fn resolved_rows(text: &str, block: &BlockNode) -> Result<Vec<Vec<Range<usize>>>, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    absolute_rows(text, block).ok_or_else(|| FormEditError::UnsupportedEditOperation {
        reason: "table did not parse as a single table block".into(),
    })
}

/// Inserts a new, fully empty row above or below row `at` (RFC-048 slice
/// 3 §2.1). `at` is a row index in `ReplaceTableCell`'s sense (0 is the
/// header); `Above` the header is refused. Inserting below the table's
/// own last row is exactly [`resolve_add_table_row`]'s own append (the
/// file may have no trailing newline there at all); every other case is
/// a pure point-insertion right before the next row's own line, so that
/// row -- and everything else -- is byte-identical afterward.
pub fn resolve_insert_table_row(
    text: &str,
    block: &BlockNode,
    at: usize,
    position: TableRowPosition,
) -> Result<Resolved, FormEditError> {
    if position == TableRowPosition::Above && at == 0 {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "cannot insert a row above the header".into(),
        });
    }
    let rows = resolved_rows(text, block)?;
    if at >= rows.len() {
        return Err(FormEditError::ItemNotFound { ordinal: at as u32 });
    }
    let col_count = rows.first().map(Vec::len).unwrap_or(0);
    let le = LineEnding::detect(text).as_str();
    let new_row = empty_row_text(col_count);
    let last_row = rows.len() - 1;

    let before_row = match position {
        TableRowPosition::Above => at,
        TableRowPosition::Below => at + 1,
    };

    let (range, replacement) = if before_row > last_row {
        // Appending after the table's own last row: there is no
        // following row's line to anchor before.
        let insert_at = block.source_range.end;
        (insert_at..insert_at, format!("{le}{new_row}"))
    } else {
        let (start, _) =
            row_line_span(text, &rows, before_row).ok_or(FormEditError::ItemNotFound {
                ordinal: before_row as u32,
            })?;
        (start..start, format!("{new_row}{le}"))
    };

    let new_row_index = before_row;
    let unchanged: Vec<(usize, Vec<String>)> = rows
        .iter()
        .enumerate()
        .map(|(idx, cells)| {
            let new_idx = if idx >= new_row_index { idx + 1 } else { idx };
            (new_idx, cell_texts(text, cells))
        })
        .collect();
    verify_row_operation(
        text,
        block,
        &range,
        &replacement,
        rows.len() + 1,
        &unchanged,
    )?;

    Ok((
        ByteRange::new(range.start, range.end),
        replacement,
        PatchOrigin::FormMode,
    ))
}

/// Deletes row `row`'s own line and its trailing line ending only
/// (RFC-048 slice 3 §2.1). The header (row 0) is refused. At the file's
/// own last line with no trailing newline, the *preceding* line ending
/// is removed instead, so the new last line is not left dangling with a
/// newline it never had.
pub fn resolve_delete_table_row(
    text: &str,
    block: &BlockNode,
    row: usize,
) -> Result<Resolved, FormEditError> {
    if row == 0 {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "the header row cannot be deleted".into(),
        });
    }
    let rows = resolved_rows(text, block)?;
    let (start, content_end) =
        row_line_span(text, &rows, row).ok_or(FormEditError::ItemNotFound {
            ordinal: row as u32,
        })?;
    let trailing = line_ending_after(text, content_end);
    let (range, replacement) = if !trailing.is_empty() {
        (start..trailing.end, String::new())
    } else {
        let preceding = line_ending_before(text, start);
        (preceding.start..content_end, String::new())
    };

    let unchanged: Vec<(usize, Vec<String>)> = rows
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != row)
        .map(|(idx, cells)| {
            let new_idx = if idx > row { idx - 1 } else { idx };
            (new_idx, cell_texts(text, cells))
        })
        .collect();
    verify_row_operation(
        text,
        block,
        &range,
        &replacement,
        rows.len() - 1,
        &unchanged,
    )?;

    Ok((
        ByteRange::new(range.start, range.end),
        replacement,
        PatchOrigin::FormMode,
    ))
}

/// Swaps row `row` with its neighbour in `direction` (RFC-048 slice 3
/// §2.1). Refuses if that neighbour would be the header, or does not
/// exist. Each line's own bytes are kept exactly; the line ending
/// between the two swapped lines, and the one after the later line, both
/// stay exactly where they were -- only the two lines' own content
/// changes which position it occupies.
pub fn resolve_move_table_row(
    text: &str,
    block: &BlockNode,
    row: usize,
    direction: TableRowDirection,
) -> Result<Resolved, FormEditError> {
    if row == 0 {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "the header row cannot move".into(),
        });
    }
    let rows = resolved_rows(text, block)?;
    let other = match direction {
        TableRowDirection::Up => row.checked_sub(1).filter(|r| *r >= 1),
        TableRowDirection::Down => row.checked_add(1).filter(|r| *r < rows.len()),
    }
    .ok_or_else(|| FormEditError::UnsupportedEditOperation {
        reason: "cannot move the row past the header or past the table's own last row".into(),
    })?;

    let (first_idx, second_idx) = (row.min(other), row.max(other));
    let (first_start, first_end) =
        row_line_span(text, &rows, first_idx).ok_or(FormEditError::ItemNotFound {
            ordinal: first_idx as u32,
        })?;
    let (second_start, second_end) =
        row_line_span(text, &rows, second_idx).ok_or(FormEditError::ItemNotFound {
            ordinal: second_idx as u32,
        })?;
    // Always non-empty: `second_idx` is a real row following `first_idx`
    // within the same table block, so there must be a line ending
    // between them.
    let between = line_ending_after(text, first_end);

    let range = first_start..second_end;
    let replacement = format!(
        "{}{}{}",
        &text[second_start..second_end],
        &text[between.clone()],
        &text[first_start..first_end],
    );

    let unchanged: Vec<(usize, Vec<String>)> = rows
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != first_idx && *idx != second_idx)
        .map(|(idx, cells)| (idx, cell_texts(text, cells)))
        .collect();
    verify_row_operation(text, block, &range, &replacement, rows.len(), &unchanged)?;

    Ok((
        ByteRange::new(range.start, range.end),
        replacement,
        PatchOrigin::FormMode,
    ))
}
