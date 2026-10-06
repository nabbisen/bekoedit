//! Tidy table (RFC-048 slice 5 §2.1): the one operation that rewrites a whole
//! table, and only when the user asks for it.
//!
//! It changes whitespace and delimiter dashes, and nothing else: every cell's
//! own text (escapes included), every alignment colon, each line's pipe style
//! and indentation, and every line ending are kept. A cell is padded to its
//! column's display width, where an East Asian wide character counts as 2
//! (`unicode-width`), so the pipes line up on screen. The result is re-parsed
//! and checked against the cell model; it is refused unless it matches.

use pulldown_cmark::Alignment;
use unicode_width::UnicodeWidthStr;

use crate::block::{BlockKind, BlockNode};
use crate::form::FormEditError;
use crate::form::columns::{Split, cell_text, refuse, split, split_lines, verify};
use crate::gfm::{table_alignments, table_cell_ranges};
use crate::patch::PatchOrigin;
use crate::range::ByteRange;

type Resolved = (ByteRange, String, PatchOrigin);

/// The tidied source, with the cell model and alignments it must re-parse to.
type Tidied = (String, Vec<Vec<String>>, Vec<Alignment>);

/// A column is never narrower than a delimiter cell can be.
const MIN_WIDTH: usize = 3;

fn delimiter_core(raw: &str, width: usize) -> String {
    let core = raw.trim();
    let left = core.starts_with(':');
    let right = core.len() > 1 && core.ends_with(':');
    let dashes = width.saturating_sub(usize::from(left) + usize::from(right));
    format!(
        "{}{}{}",
        if left { ":" } else { "" },
        "-".repeat(dashes),
        if right { ":" } else { "" }
    )
}

/// One line with its cells re-padded; cells past the header's width, and a
/// short row's absent cells, are left exactly as they are.
fn tidy_line(line: &str, m: &Split, widths: &[usize], is_delimiter: bool) -> String {
    let count = m.cells.len();
    let mut inners = Vec::with_capacity(count);
    for (i, span) in m.cells.iter().enumerate() {
        let raw = &line[span.clone()];
        let Some(&width) = widths.get(i) else {
            inners.push(raw.to_string());
            continue;
        };
        let inner = if is_delimiter {
            format!(" {} ", delimiter_core(raw, width))
        } else {
            let text = raw.trim();
            let pad = width.saturating_sub(UnicodeWidthStr::width(text));
            format!(" {text}{} ", " ".repeat(pad))
        };
        inners.push(inner);
    }
    if m.leading.is_none() {
        inners[0] = inners[0].trim_start().to_string();
    }
    if m.trailing.is_none() {
        inners[count - 1] = inners[count - 1].trim_end().to_string();
    }
    let prefix = m.leading.map_or("", |l| &line[..=l]);
    let suffix = m.trailing.map_or("", |t| &line[t..]);
    format!("{prefix}{}{suffix}", inners.join("|"))
}

/// The tidied source, together with the cell model it must re-parse to.
fn tidy_source(source: &str) -> Result<Tidied, FormEditError> {
    let rows = table_cell_ranges(source).ok_or_else(|| refuse("table did not parse"))?;
    let alignments = table_alignments(source).ok_or_else(|| refuse("table did not parse"))?;
    let h = alignments.len();
    let lines = split_lines(source);
    if lines.len() != rows.len() + 1 {
        return Err(refuse("table lines do not match its rows"));
    }
    let mut splits = Vec::with_capacity(lines.len());
    let mut model = Vec::with_capacity(rows.len());
    let mut widths = vec![MIN_WIDTH; h];
    for (idx, (content, _)) in lines.iter().enumerate() {
        let m = split(content).ok_or_else(|| refuse("a table line has no content"))?;
        if idx == 1 {
            if m.cells.len() != h {
                return Err(refuse("the delimiter row disagrees with the parser"));
            }
        } else {
            let row = idx.saturating_sub(1);
            if rows[row].len() != h {
                return Err(refuse("the pipe split disagrees with the parser"));
            }
            model.push(m.cells.iter().map(|r| cell_text(content, r)).collect());
            for (c, span) in m.cells.iter().take(h).enumerate() {
                widths[c] = widths[c].max(UnicodeWidthStr::width(content[span.clone()].trim()));
            }
        }
        splits.push(m);
    }
    let mut out = String::with_capacity(source.len());
    for (idx, (content, line_ending)) in lines.iter().enumerate() {
        out.push_str(&tidy_line(content, &splits[idx], &widths, idx == 1));
        out.push_str(line_ending);
    }
    Ok((out, model, alignments))
}

/// Whether tidying would change nothing: the Tidy action's own no-op test,
/// shown as a disabled state and used to keep an unchanged table from being
/// "edited" (a revision bump and a dirty document for the same bytes).
pub fn table_is_tidy(source: &str) -> bool {
    tidy_source(source).is_ok_and(|(tidied, ..)| tidied == source)
}

/// Resolves Tidy against a simple table (RFC-048 slice 5 §2.1).
pub fn resolve_tidy_table(text: &str, block: &BlockNode) -> Result<Resolved, FormEditError> {
    if block.kind != BlockKind::SimpleTable {
        return Err(refuse("not a simple table"));
    }
    let source = &text[block.source_range.start..block.source_range.end];
    let (tidied, model, alignments) = tidy_source(source)?;
    verify(&tidied, &model, &alignments)?;
    Ok((
        ByteRange::new(block.source_range.start, block.source_range.end),
        tidied,
        PatchOrigin::FormMode,
    ))
}
