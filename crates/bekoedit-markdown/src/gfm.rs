//! Whether Markdown holds a GFM table (RFC-046 §5.4), and shared,
//! low-level table-cell helpers used by both the indexer's classifier
//! (`index/blocks.rs`) and Form Mode's table editor (`form.rs`,
//! `form/tables.rs`) -- this module sits below both, so neither depends
//! on the other through it.
//!
//! Slice 2 of the paste work raises "pasted table as text: no Markdown table
//! form" when the pasted HTML had a `<table` and the converted Markdown has no
//! table. This is the output check, and it parses with the document's own
//! options, so it agrees with what the editor and Preview will call a table.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::index::parse_options;

/// True when `markdown` contains at least one GFM table, at any depth: in a
/// blockquote, a list item, or after other content. A `|` in prose, a table in a
/// code block, and a pipe row with no delimiter row are not tables.
pub fn has_gfm_table(markdown: &str) -> bool {
    Parser::new_ext(markdown, parse_options())
        .any(|event| matches!(event, Event::Start(Tag::Table(_))))
}

/// Each row's cell byte ranges within `source` (one table block's own
/// text, whether classified `SimpleTable` or not yet classified at all),
/// the header first, then each data row, in source order -- trimmed of
/// each cell's own leading and trailing whitespace. `None` if `source`
/// does not parse as a single well-formed table.
///
/// Cell boundaries come directly from `pulldown-cmark`'s own `TableCell`
/// event ranges (task 045 review, 2026-10-02): a `Start`/`End` pair for
/// one cell carries the same range regardless of what inline content is
/// inside it (plain text, a link, an image, an autolink, an entity, bold,
/// code -- confirmed empirically before relying on it), so this function
/// does not care what formatting a cell holds, only where it starts and
/// ends. That is what lets the indexer's classifier
/// (`index::blocks::classify_table`, RFC-048 slice 2) use the exact same
/// function as the editor and the projection to decide whether a table's
/// cells can be found at all: a table is `SimpleTable` exactly when this
/// returns `Some`.
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
/// rule) -- the inverse of `form::tables`'s private `escape_pipe`.
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
