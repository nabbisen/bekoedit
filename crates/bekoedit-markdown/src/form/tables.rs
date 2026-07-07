//! Table block resolution for Form Mode (RFC-027).

use crate::block::BlockNode;
use crate::form::FormEditError;
use crate::patch::PatchOrigin;
use crate::range::ByteRange;
use crate::trivia::LineEnding;

type Resolved = (ByteRange, String, PatchOrigin);
/// Parses a GFM table source into rows of cell texts and their source ranges.
/// Returns `(header_row, data_rows)` where each entry is a Vec of cell strings.
/// Panics: never. Returns empty on malformed tables.
fn parse_table_cells(source: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let lines: Vec<&str> = source.lines().collect();
    let parse_row = |line: &str| -> Vec<String> {
        let trimmed = line.trim().trim_start_matches('|').trim_end_matches('|');
        trimmed.split('|').map(|c| c.trim().to_string()).collect()
    };
    let is_separator = |line: &str| {
        let t = line.trim();
        t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && t.contains('-')
    };

    let mut rows = lines
        .iter()
        .filter(|l| !is_separator(l))
        .map(|l| parse_row(l));
    let headers = rows.next().unwrap_or_default();
    let data: Vec<Vec<String>> = rows.collect();
    (headers, data)
}

/// Regenerates a GFM table from headers and rows, using the original
/// column widths (widened if needed, never narrowed to preserve readability).
fn render_table(headers: &[String], rows: &[Vec<String>], line_ending: &str) -> String {
    let col_count = headers.len();
    // Compute column widths.
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len().max(3)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let fmt_row = |cells: &[String]| -> String {
        let mut s = String::from("|");
        for (i, cell) in cells.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(3);
            s.push(' ');
            s.push_str(cell);
            for _ in cell.len()..w {
                s.push(' ');
            }
            s.push_str(" |");
        }
        s
    };
    let separator: String = widths
        .iter()
        .map(|&w| format!(" {:-<w$} |", "", w = w))
        .collect::<String>();
    let separator = format!("|{separator}");
    let mut out = fmt_row(headers);
    out.push_str(line_ending);
    out.push_str(&separator);
    for row in rows {
        out.push_str(line_ending);
        // Pad short rows.
        let padded: Vec<String> = (0..col_count)
            .map(|i| row.get(i).cloned().unwrap_or_default())
            .collect();
        out.push_str(&fmt_row(&padded));
    }
    out
}

pub fn resolve_replace_table_cell(
    text: &str,
    block: &BlockNode,
    row: usize,
    col: usize,
    cell_text: &str,
) -> Result<Resolved, crate::form::FormEditError> {
    if block.kind != crate::block::BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let le = LineEnding::detect(text).as_str();
    let (mut headers, mut rows) = parse_table_cells(source);
    if row == 0 {
        if col < headers.len() {
            headers[col] = cell_text.to_string();
        } else {
            return Err(FormEditError::ItemNotFound {
                ordinal: col as u32,
            });
        }
    } else {
        let data_row = row - 1;
        if data_row < rows.len() && col < rows[data_row].len() {
            rows[data_row][col] = cell_text.to_string();
        } else {
            return Err(FormEditError::ItemNotFound {
                ordinal: row as u32,
            });
        }
    }
    Ok((
        block.source_range,
        render_table(&headers, &rows, le),
        PatchOrigin::FormMode,
    ))
}

pub fn resolve_add_table_row(
    text: &str,
    block: &BlockNode,
) -> Result<Resolved, crate::form::FormEditError> {
    if block.kind != crate::block::BlockKind::SimpleTable {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a simple table".into(),
        });
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let le = LineEnding::detect(text).as_str();
    let (headers, mut rows) = parse_table_cells(source);
    let empty_row = vec![String::new(); headers.len()];
    rows.push(empty_row);
    Ok((
        block.source_range,
        render_table(&headers, &rows, le),
        PatchOrigin::FormMode,
    ))
}
