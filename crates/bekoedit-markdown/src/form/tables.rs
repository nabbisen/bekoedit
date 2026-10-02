//! Table block resolution for Form Mode (RFC-027).
//!
//! Task 045: a cell edit changes only that cell's own content range. The
//! table is never regenerated -- every other byte (the separator line's
//! alignment markers, every other cell, this cell's own surrounding
//! whitespace, line endings) is left exactly as the user wrote it.
//!
//! Cell boundaries are found by splitting each row's line on unescaped
//! `|` (GFM table syntax: a leading and a trailing pipe are each
//! optional; `\|` is part of a cell's content, never a delimiter), then
//! cross-checked against what `pulldown-cmark` itself parses from the
//! same source. **The oracle is pulldown-cmark**: if the two disagree,
//! in row count, cell count, or any cell's text, the edit is refused
//! with `UnsupportedEditOperation` rather than guessed at.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::block::{BlockKind, BlockNode};
use crate::form::FormEditError;
use crate::index::parse_options;
use crate::patch::PatchOrigin;
use crate::range::ByteRange;
use crate::trivia::LineEnding;

type Resolved = (ByteRange, String, PatchOrigin);

/// Byte ranges, within `source`, of each table row's own line -- the
/// header first, then each data row, in source order -- excluding the
/// GFM separator line (e.g. `|---|:-:|--:|`) and excluding each line's
/// own line-ending bytes.
fn table_row_lines(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut lines = Vec::new();
    let mut start = 0;
    loop {
        let mut end = start;
        while end < len && bytes[end] != b'\n' {
            end += 1;
        }
        let content_end = if end > start && bytes[end - 1] == b'\r' {
            end - 1
        } else {
            end
        };
        let line = &source[start..content_end];
        if !line.trim().is_empty() && !is_separator_line(line) {
            lines.push(start..content_end);
        }
        if end >= len {
            break;
        }
        start = end + 1;
    }
    lines
}

fn is_separator_line(line: &str) -> bool {
    let t = line.trim();
    t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && t.contains('-')
}

/// Byte ranges, within `line` (one table row's own line, as returned by
/// [`table_row_lines`]), of each cell's own content, trimmed of its own
/// leading and trailing whitespace. An empty cell (`||` or `| |`) yields
/// a zero-length range positioned exactly where its text would be typed
/// in: right after any leading whitespace the cell already has, or
/// directly after the pipe when it has none -- this falls out of
/// trimming the leading side fully before the trailing side, with no
/// cell-emptiness special case needed.
fn cell_ranges_in_line(line: &str) -> Vec<Range<usize>> {
    let bytes = line.as_bytes();
    let len = bytes.len();

    let mut region_start = 0;
    while region_start < len && bytes[region_start].is_ascii_whitespace() {
        region_start += 1;
    }
    let mut region_end = len;
    while region_end > region_start && bytes[region_end - 1].is_ascii_whitespace() {
        region_end -= 1;
    }
    if region_start < region_end && bytes[region_start] == b'|' {
        region_start += 1;
    }
    if region_end > region_start && bytes[region_end - 1] == b'|' {
        region_end -= 1;
    }

    let mut cells = Vec::new();
    let mut cell_start = region_start;
    let mut i = region_start;
    while i < region_end {
        if bytes[i] == b'\\' && i + 1 < region_end {
            i += 2;
            continue;
        }
        if bytes[i] == b'|' {
            cells.push(cell_start..i);
            i += 1;
            cell_start = i;
            continue;
        }
        i += 1;
    }
    cells.push(cell_start..region_end);

    cells
        .into_iter()
        .map(|r| trim_cell_range(line, r))
        .collect()
}

fn trim_cell_range(line: &str, range: Range<usize>) -> Range<usize> {
    let bytes = line.as_bytes();
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
/// rule) -- the inverse of [`escape_pipe`].
fn unescape_pipe(cell_source: &str) -> String {
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

/// What `pulldown-cmark` itself parses `source` as: each row's cells'
/// text, the header first, then each data row, in source order. `None`
/// if the source does not parse as the single, well-formed table
/// structure this function expects (an unterminated row or cell can
/// only come from a parse this module does not understand).
fn pulldown_table_texts(source: &str) -> Option<Vec<Vec<String>>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Option<Vec<String>> = None;
    let mut current_cell: Option<String> = None;
    for (event, _range) in Parser::new_ext(source, parse_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                current_row = Some(Vec::new());
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                rows.push(current_row.take()?);
            }
            Event::Start(Tag::TableCell) => {
                current_cell = Some(String::new());
            }
            Event::End(TagEnd::TableCell) => {
                current_row.as_mut()?.push(current_cell.take()?);
            }
            // A SimpleTable's cells hold only Event::Text (classify_table
            // demotes anything with inline markup to ComplexTable), but a
            // cell with an escaped pipe splits into more than one Text
            // event around it -- both must be concatenated to get the
            // cell's full rendered text.
            Event::Text(text) => {
                if let Some(cell) = current_cell.as_mut() {
                    cell.push_str(&text);
                }
            }
            _ => {}
        }
    }
    if rows.is_empty() { None } else { Some(rows) }
}

/// Parses `source` (one `SimpleTable` block's own text) into each row's
/// cell byte ranges, cross-checked against [`pulldown_table_texts`] --
/// this module's one entry point into both the splitter and the oracle,
/// per the module doc comment.
fn verified_cell_ranges(source: &str) -> Result<Vec<Vec<Range<usize>>>, FormEditError> {
    let refuse = |detail: String| FormEditError::UnsupportedEditOperation { reason: detail };

    let row_lines = table_row_lines(source);
    let cell_ranges: Vec<Vec<Range<usize>>> = row_lines
        .iter()
        .map(|line_range| {
            cell_ranges_in_line(&source[line_range.clone()])
                .into_iter()
                .map(|r| (r.start + line_range.start)..(r.end + line_range.start))
                .collect()
        })
        .collect();

    let oracle = pulldown_table_texts(source)
        .ok_or_else(|| refuse("table did not parse as a single table block".into()))?;

    if oracle.len() != cell_ranges.len() {
        return Err(refuse(format!(
            "cell splitter disagrees with pulldown-cmark on row count: {} vs {}",
            cell_ranges.len(),
            oracle.len()
        )));
    }
    for (row_idx, (own_row, oracle_row)) in cell_ranges.iter().zip(&oracle).enumerate() {
        if own_row.len() != oracle_row.len() {
            return Err(refuse(format!(
                "cell splitter disagrees with pulldown-cmark on row {row_idx}'s cell count: \
                 {} vs {}",
                own_row.len(),
                oracle_row.len()
            )));
        }
        for (col_idx, (range, oracle_text)) in own_row.iter().zip(oracle_row).enumerate() {
            let own_text = unescape_pipe(&source[range.clone()]);
            if &own_text != oracle_text {
                return Err(refuse(format!(
                    "cell splitter disagrees with pulldown-cmark on row {row_idx} col {col_idx}'s text"
                )));
            }
        }
    }
    Ok(cell_ranges)
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
    let rows = verified_cell_ranges(source)?;
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
    let rows = verified_cell_ranges(source)?;
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
