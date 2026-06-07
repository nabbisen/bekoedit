//! Safe Preview Mode rendering (RFC-012).
//!
//! Security policy (requirements §17.2, external design §29.2):
//! raw HTML found in Markdown — both block and inline — is escaped and
//! displayed literally instead of being injected into the DOM. Scripts
//! therefore can never execute from document content.

use pulldown_cmark::{Event, Options, Parser, html};

use crate::index::detect_front_matter;

/// Renders canonical Markdown into sanitized HTML for the read-only
/// preview surface. Front matter is skipped from the rendered output
/// (it is shown as a Raw Markdown Island in Form Mode instead).
pub fn render_preview_html(text: &str) -> String {
    let body_offset = detect_front_matter(text).unwrap_or(0);
    let body = &text[body_offset..];
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    let parser = Parser::new_ext(body, options).map(|event| match event {
        // `Text` events are HTML-escaped by `push_html`, so converting raw
        // HTML into text displays it verbatim without executing it.
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}
