//! Column operations on a simple GFM table (RFC-048 slice 4).
//!
//! Each operation is one source patch over the table block. Every line is
//! rebuilt by splicing only its own cells, so every byte outside the edited
//! cells, including every line ending, is copied unchanged. The result is
//! then re-parsed, and it is refused, never applied, unless it parses back
//! to the cell texts and alignments the operation names.
//!
//! The delimiter row is not one of `table_cell_ranges`' rows, so its cells
//! are split here by the same unescaped-pipe rule as every other line. Its
//! content is only `:`, `-`, spaces and pipes, so no escape or inline markup
//! can make that split disagree with the parser, and `table_alignments`
//! confirms the column count.

use std::ops::Range;

use pulldown_cmark::Alignment;

use crate::block::{BlockKind, BlockNode};
use crate::form::{FormEditError, TableAlignment, TableColumnDirection, TableColumnPosition};
use crate::gfm::{table_alignments, table_cell_ranges, unescape_pipe};
use crate::patch::PatchOrigin;
use crate::range::ByteRange;

type Resolved = (ByteRange, String, PatchOrigin);

/// What a column operation does, after the UI's column index is checked.
#[derive(Debug, Clone, Copy)]
pub enum ColumnOp {
    Insert {
        col: usize,
        position: TableColumnPosition,
    },
    Delete {
        col: usize,
    },
    Move {
        col: usize,
        direction: TableColumnDirection,
    },
    SetAlignment {
        col: usize,
        alignment: TableAlignment,
    },
}

/// The new cell's inner text, before the line's own pipe: two spaces for a
/// header or data cell (an empty cell, as slice 3 settled), and ` --- ` for
/// the delimiter, which makes a `---` cell with one space each side.
const EMPTY_CELL: &str = "  ";
const NEW_DELIMITER_CELL: &str = " --- ";

/// One line's own pipe structure: its outer pipes, its inner separators, and
/// the raw span of every cell it holds, in order.
struct Split {
    leading: Option<usize>,
    trailing: Option<usize>,
    seps: Vec<usize>,
    cells: Vec<Range<usize>>,
}

/// Unescaped `|` positions in one line. A backslash skips the byte it
/// escapes, so `\|` is never a boundary and `\\|` is one.
fn pipes(line: &str) -> Vec<usize> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'|' => {
                out.push(i);
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// `None` for a line with no content, which is not a table row.
fn split(line: &str) -> Option<Split> {
    let first = line.find(|c: char| !c.is_whitespace())?;
    let last = line.rfind(|c: char| !c.is_whitespace())?;
    let all = pipes(line);
    let leading = (line.as_bytes()[first] == b'|' && all.contains(&first)).then_some(first);
    let trailing =
        (last != first && line.as_bytes()[last] == b'|' && all.contains(&last)).then_some(last);
    let seps: Vec<usize> = all
        .iter()
        .copied()
        .filter(|i| Some(*i) != leading && Some(*i) != trailing)
        .collect();
    let count = seps.len() + 1;
    let cells = (0..count)
        .map(|i| {
            let start = if i == 0 {
                leading.map_or(0, |l| l + 1)
            } else {
                seps[i - 1] + 1
            };
            let end = if i == count - 1 {
                trailing.unwrap_or(line.len())
            } else {
                seps[i]
            };
            start..end
        })
        .collect();
    Some(Split {
        leading,
        trailing,
        seps,
        cells,
    })
}

/// The cell text the projection shows: trimmed, with only `\|` unescaped
/// (task 045 §2.2), so the model below compares what a user reads.
fn cell_text(line: &str, span: &Range<usize>) -> String {
    unescape_pipe(line[span.clone()].trim())
}

fn splice(line: &str, range: Range<usize>, with: &str) -> String {
    format!("{}{}{}", &line[..range.start], with, &line[range.end..])
}

/// A new cell at position `p`, a column index in `0..=cells`. An empty cell
/// (`contentful` is false) is only a cell if a pipe closes it, so at a line's
/// edge one is added where the line had none; a delimiter cell has content,
/// so it needs only the separator (`contentful` is true).
fn insert_cell(line: &str, m: &Split, p: usize, inner: &str, contentful: bool) -> String {
    let count = m.cells.len();
    let (at, text) = if p == 0 {
        match m.leading {
            Some(l) => (l + 1, format!("{inner}|")),
            None if contentful => (0, format!("{inner}|")),
            None => (0, format!("|{inner}|")),
        }
    } else if p < count {
        (m.seps[p - 1] + 1, format!("{inner}|"))
    } else if m.trailing.is_some() {
        (m.trailing.unwrap() + 1, format!("{inner}|"))
    } else if contentful {
        (line.len(), format!(" |{inner}"))
    } else {
        (line.len(), format!(" |{inner}|"))
    };
    splice(line, at..at, &text)
}

/// Removes column `c` and exactly one adjacent pipe. Needs at least two cells.
fn delete_cell(line: &str, m: &Split, c: usize) -> String {
    let count = m.cells.len();
    let range = if c + 1 < count {
        if c == 0 {
            m.cells[0].start..m.seps[0] + 1
        } else {
            m.seps[c - 1] + 1..m.seps[c] + 1
        }
    } else {
        m.seps[c - 1]..m.cells[c].end
    };
    splice(line, range, "")
}

/// Swaps two adjacent cells, leaving the separator between them in place.
fn swap_cells(line: &str, m: &Split, a: usize, b: usize) -> String {
    let (lo, hi) = (a.min(b), a.max(b));
    let (l, h) = (&m.cells[lo], &m.cells[hi]);
    format!(
        "{}{}{}{}{}",
        &line[..l.start],
        &line[h.clone()],
        &line[l.end..h.start],
        &line[l.clone()],
        &line[h.end..],
    )
}

/// Rewrites one delimiter cell's core, keeping its own padding and dash
/// count where the form allows it, with three dashes at least.
fn set_alignment(line: &str, m: &Split, c: usize, alignment: TableAlignment) -> Option<String> {
    let span = m.cells[c].clone();
    let raw = &line[span.clone()];
    let core = raw.trim();
    if core.is_empty() || !core.contains('-') || !core.chars().all(|ch| ch == '-' || ch == ':') {
        return None;
    }
    let dashes = "-".repeat(core.matches('-').count().max(3));
    let new_core = match alignment {
        TableAlignment::None => dashes,
        TableAlignment::Left => format!(":{dashes}"),
        TableAlignment::Centre => format!(":{dashes}:"),
        TableAlignment::Right => format!("{dashes}:"),
    };
    let lead = raw.len() - raw.trim_start().len();
    let trail = raw.len() - raw.trim_end().len();
    let new_raw = format!("{}{new_core}{}", &raw[..lead], &raw[raw.len() - trail..]);
    Some(splice(line, span, &new_raw))
}

/// Every line of `source` as `(content, line ending)`, so that joining
/// `content + line ending` for all lines gives `source` back exactly.
fn split_lines(source: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, _) in source.match_indices('\n') {
        let content_end = if i > start && source.as_bytes()[i - 1] == b'\r' {
            i - 1
        } else {
            i
        };
        out.push((&source[start..content_end], &source[content_end..=i]));
        start = i + 1;
    }
    if start < source.len() {
        out.push((&source[start..], ""));
    }
    out
}

fn refuse(reason: &str) -> FormEditError {
    FormEditError::UnsupportedEditOperation {
        reason: reason.to_string(),
    }
}

/// The cell texts each row has after `op`, computed from the cell model alone,
/// together with the alignments after it. This is the oracle the re-parsed
/// result is checked against.
fn expected_model(
    rows: &[Vec<String>],
    alignments: &[Alignment],
    op: ColumnOp,
) -> Result<(Vec<Vec<String>>, Vec<Alignment>), FormEditError> {
    let mut alignments: Vec<Alignment> = alignments.to_vec();
    let mut out: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    let h = alignments.len();
    match op {
        ColumnOp::Insert { col, position } => {
            let p = match position {
                TableColumnPosition::Left => col,
                TableColumnPosition::Right => col + 1,
            };
            for row in rows {
                let mut row = row.clone();
                if row.len() > col {
                    row.insert(p.min(row.len()), String::new());
                }
                out.push(row);
            }
            alignments.insert(p, Alignment::None);
        }
        ColumnOp::Delete { col } => {
            for row in rows {
                let mut row = row.clone();
                if row.len() > col {
                    row.remove(col);
                }
                out.push(row);
            }
            alignments.remove(col);
        }
        ColumnOp::Move { col, direction } => {
            let neighbour = match direction {
                TableColumnDirection::Left => col.checked_sub(1),
                TableColumnDirection::Right => Some(col + 1),
            }
            .filter(|n| *n < h)
            .ok_or_else(|| refuse("a column cannot move past the first or last column"))?;
            let (lo, hi) = (col.min(neighbour), col.max(neighbour));
            for row in rows {
                let mut row = row.clone();
                if row.len() > hi {
                    row.swap(lo, hi);
                }
                out.push(row);
            }
            alignments.swap(lo, hi);
        }
        ColumnOp::SetAlignment { col, alignment } => {
            out = rows.to_vec();
            alignments[col] = match alignment {
                TableAlignment::None => Alignment::None,
                TableAlignment::Left => Alignment::Left,
                TableAlignment::Centre => Alignment::Center,
                TableAlignment::Right => Alignment::Right,
            };
        }
    }
    Ok((out, alignments))
}

/// Resolves one column operation against a simple table (RFC-048 slice 4).
pub fn resolve_column_op(
    text: &str,
    block: &BlockNode,
    op: ColumnOp,
) -> Result<Resolved, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(refuse("not a simple table"));
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let rows = table_cell_ranges(source).ok_or_else(|| refuse("table did not parse"))?;
    let alignments = table_alignments(source).ok_or_else(|| refuse("table did not parse"))?;
    let h = alignments.len();

    let (col, is_bounded) = match op {
        ColumnOp::Insert { col, .. } => (col, col < h),
        ColumnOp::Delete { col } | ColumnOp::SetAlignment { col, .. } => (col, col < h),
        ColumnOp::Move { col, .. } => (col, col < h),
    };
    if !is_bounded {
        return Err(FormEditError::ItemNotFound {
            ordinal: col as u32,
        });
    }
    if matches!(op, ColumnOp::Delete { .. }) && h == 1 {
        return Err(refuse("the last remaining column cannot be deleted"));
    }
    // With no outer pipes, one column's line is just its text, and `text` over
    // `-` is a setext heading, not a table: a table that keeps one column must
    // keep its pipes.
    if let ColumnOp::Delete { .. } = op
        && h == 2
        && let Some(header) = split_lines(source)
            .first()
            .and_then(|(content, _)| split(content))
        && header.leading.is_none()
        && header.trailing.is_none()
    {
        return Err(refuse(
            "deleting a column would leave a table with no pipes to hold it",
        ));
    }

    let lines = split_lines(source);
    if lines.len() != rows.len() + 1 {
        return Err(refuse("table lines do not match its rows"));
    }
    // Header and data rows, as the cell model reads them.
    let mut splits: Vec<Split> = Vec::with_capacity(lines.len());
    let mut row_texts: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    for (idx, (content, _)) in lines.iter().enumerate() {
        let m = split(content).ok_or_else(|| refuse("a table line has no content"))?;
        if idx == 1 {
            if m.cells.len() != h {
                return Err(refuse("the delimiter row disagrees with the parser"));
            }
        } else {
            let row = if idx == 0 { 0 } else { idx - 1 };
            if rows[row].len() != h {
                return Err(refuse("the pipe split disagrees with the parser"));
            }
            row_texts.push(m.cells.iter().map(|r| cell_text(content, r)).collect());
        }
        splits.push(m);
    }
    let (expected_rows, expected_alignments) = expected_model(&row_texts, &alignments, op)?;

    // Rebuild every line from its own cells.
    let mut rebuilt = String::with_capacity(source.len() + 16);
    for (idx, (content, line_ending)) in lines.iter().enumerate() {
        let m = &splits[idx];
        let new_content = if idx == 1 {
            delimiter_line(content, m, op)?
        } else {
            let k = m.cells.len();
            match op {
                ColumnOp::Insert { col, position } if k > col => {
                    let p = match position {
                        TableColumnPosition::Left => col,
                        TableColumnPosition::Right => col + 1,
                    };
                    insert_cell(content, m, p, EMPTY_CELL, false)
                }
                ColumnOp::Delete { col } if k > col => {
                    if k == 1 {
                        return Err(refuse("a row with one cell cannot lose its only cell"));
                    }
                    delete_cell(content, m, col)
                }
                ColumnOp::Move { col, direction } => {
                    let neighbour = match direction {
                        TableColumnDirection::Left => col - 1,
                        TableColumnDirection::Right => col + 1,
                    };
                    if k > col.max(neighbour) {
                        swap_cells(content, m, col, neighbour)
                    } else {
                        (*content).to_string()
                    }
                }
                _ => (*content).to_string(),
            }
        };
        rebuilt.push_str(&new_content);
        rebuilt.push_str(line_ending);
    }

    verify(&rebuilt, &expected_rows, &expected_alignments)?;

    Ok((
        ByteRange::new(block.source_range.start, block.source_range.end),
        rebuilt,
        PatchOrigin::FormMode,
    ))
}

/// The delimiter row after `op`: always affected by a column insert, delete
/// or move, since it has a cell for every column.
fn delimiter_line(content: &str, m: &Split, op: ColumnOp) -> Result<String, FormEditError> {
    Ok(match op {
        ColumnOp::Insert { col, position } => {
            let p = match position {
                TableColumnPosition::Left => col,
                TableColumnPosition::Right => col + 1,
            };
            insert_cell(content, m, p, NEW_DELIMITER_CELL, true)
        }
        ColumnOp::Delete { col } => delete_cell(content, m, col),
        ColumnOp::Move { col, direction } => {
            let neighbour = match direction {
                TableColumnDirection::Left => col - 1,
                TableColumnDirection::Right => col + 1,
            };
            swap_cells(content, m, col, neighbour)
        }
        ColumnOp::SetAlignment { col, alignment } => set_alignment(content, m, col, alignment)
            .ok_or_else(|| refuse("the delimiter cell is not a delimiter"))?,
    })
}

/// Re-parses the rebuilt table and refuses it unless it is exactly the model
/// the operation named: the same rows, each with the expected cells, and the
/// expected alignments.
fn verify(
    rebuilt: &str,
    expected_rows: &[Vec<String>],
    expected_alignments: &[Alignment],
) -> Result<(), FormEditError> {
    let rows = table_cell_ranges(rebuilt)
        .ok_or_else(|| refuse("the column operation's result did not re-parse as a table"))?;
    let alignments = table_alignments(rebuilt)
        .ok_or_else(|| refuse("the column operation's result did not re-parse as a table"))?;
    if alignments != expected_alignments {
        return Err(refuse(
            "the column operation's result has the wrong alignments",
        ));
    }
    if rows.len() != expected_rows.len() {
        return Err(refuse(
            "the column operation's result has the wrong row count",
        ));
    }
    // The parser reads at most the header's width of cells in a row, padding
    // a short row and dropping a long row's extras, so only those are checked
    // here. A long row's extra cells are checked by the byte construction.
    for (row, expected) in rows.iter().zip(expected_rows) {
        if row.len() != expected_alignments.len() {
            return Err(refuse(
                "the column operation's result has the wrong cell count",
            ));
        }
        for (span, want) in row.iter().zip(expected) {
            if &cell_text(rebuilt, span) != want {
                return Err(refuse("the column operation changed an untouched cell"));
            }
        }
    }
    Ok(())
}
