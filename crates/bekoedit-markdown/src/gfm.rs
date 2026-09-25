//! Whether Markdown holds a GFM table (RFC-046 §5.4).
//!
//! Slice 2 of the paste work raises "pasted table as text: no Markdown table
//! form" when the pasted HTML had a `<table` and the converted Markdown has no
//! table. This is the output check, and it parses with the document's own
//! options, so it agrees with what the editor and Preview will call a table.

use pulldown_cmark::{Event, Parser, Tag};

use crate::index::parse_options;

/// True when `markdown` contains at least one GFM table, at any depth: in a
/// blockquote, a list item, or after other content. A `|` in prose, a table in a
/// code block, and a pipe row with no delimiter row are not tables.
pub fn has_gfm_table(markdown: &str) -> bool {
    Parser::new_ext(markdown, parse_options())
        .any(|event| matches!(event, Event::Start(Tag::Table(_))))
}
