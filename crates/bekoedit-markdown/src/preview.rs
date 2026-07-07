//! Safe Preview Mode rendering (RFC-012, RFC-038).
//!
//! Security policy (requirements §17.2): raw HTML is escaped and shown
//! verbatim. Math expressions (RFC-038, ENABLE_MATH) are shown as their
//! LaTeX source wrapped in <code> elements; a future KaTeX bundle can
//! progressively enhance this without changing the core renderer.

use pulldown_cmark::{Event, Options, Parser, html};

use crate::index::detect_front_matter;

/// Renders canonical Markdown into sanitized HTML for the read-only
/// preview surface. Front matter is skipped. Raw HTML in the source is
/// displayed escaped (scripts never execute).
pub fn render_preview_html(text: &str) -> String {
    let body_offset = detect_front_matter(text).unwrap_or(0);
    let body = &text[body_offset..];
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    let parser = Parser::new_ext(body, options).map(|event| match event {
        // Escape all raw HTML so document-controlled scripts cannot execute.
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        // Render math as readable LaTeX source (RFC-038).
        // A progressive-enhancement KaTeX pass can be layered on top later.
        Event::InlineMath(code) => {
            Event::Html(format!("<code class=\"math-inline\">{}</code>", html_escape(&code)).into())
        }
        Event::DisplayMath(code) => Event::Html(
            format!(
                "<pre class=\"math-block\"><code>{}</code></pre>\n",
                html_escape(&code)
            )
            .into(),
        ),
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
