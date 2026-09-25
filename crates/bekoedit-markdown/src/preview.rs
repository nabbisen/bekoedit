//! Safe Preview Mode rendering (RFC-012, RFC-038).
//!
//! Security policy (requirements §17.2, invariant 10): Preview never executes
//! document content. Raw HTML is escaped and shown verbatim, with **one stated
//! exception**, a bare inline `<br>` (below). Math expressions (RFC-038,
//! ENABLE_MATH) are shown as their LaTeX source wrapped in <code> elements; a
//! future KaTeX bundle can progressively enhance this without changing the
//! core renderer.
//!
//! **Link and image destinations pass an allowlist** (task 030), because the
//! Markdown syntax `[x](javascript:...)` reaches the page as a live `href`
//! even though raw HTML is escaped, and Preview injects its HTML into the
//! same WebView origin as the editor bridge:
//!
//! | Destination | Links | Images |
//! |---|---|---|
//! | `http:`, `https:` | allowed | allowed |
//! | `mailto:` | allowed | dropped |
//! | relative path, `#fragment` | allowed | allowed |
//! | `data:image/png`, `jpeg`, `gif`, `webp` | dropped | allowed |
//! | anything else | dropped | dropped |
//!
//! A dropped link renders its text as plain inline content, and a dropped
//! image its alt text, so nothing disappears from the reader's view. This is a
//! projection rule only: the source bytes are never touched.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use crate::index::detect_front_matter;

/// The destination as a browser's URL parser sees it before looking for a
/// scheme: leading C0 control characters and spaces are stripped, and ASCII
/// tab and newlines are removed everywhere. (The destination arrives here
/// already entity-decoded, so `&#106;avascript:` is `javascript:`.)
fn normalized(destination: &str) -> String {
    destination
        .trim_start_matches(|c: char| c.is_ascii_control() || c == ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// The lowercase scheme, or `None` for a relative path or a `#fragment`.
fn scheme(normalized: &str) -> Option<String> {
    let name = &normalized[..normalized.find(':')?];
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| name.to_ascii_lowercase())
}

fn link_destination_allowed(destination: &str) -> bool {
    match scheme(&normalized(destination)).as_deref() {
        None | Some("http" | "https" | "mailto") => true,
        Some(_) => false,
    }
}

fn image_destination_allowed(destination: &str) -> bool {
    let normalized = normalized(destination);
    match scheme(&normalized).as_deref() {
        None | Some("http" | "https") => true,
        Some("data") => {
            let media_type = normalized["data:".len()..]
                .split([';', ','])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            matches!(
                media_type.as_str(),
                "image/png" | "image/jpeg" | "image/gif" | "image/webp"
            )
        }
        Some(_) => false,
    }
}

/// Exactly `<br>`, `<br/>` or `<br />`, ASCII case-insensitive, and nothing
/// else. Never a prefix or a pattern: `<br class="x">` is not a bare `<br>`.
fn is_bare_br(html: &str) -> bool {
    ["<br>", "<br/>", "<br />"]
        .iter()
        .any(|bare| html.eq_ignore_ascii_case(bare))
}

/// Renders canonical Markdown into sanitized HTML for the read-only
/// preview surface. Front matter is skipped. Raw HTML in the source is
/// displayed escaped (scripts never execute), except a bare inline `<br>`;
/// link and image destinations pass the allowlist above.
pub fn render_preview_html(text: &str) -> String {
    let body_offset = detect_front_matter(text).unwrap_or(0);
    let body = &text[body_offset..];
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    // One entry per open link or image: whether its tags were dropped, so the
    // matching end tag is dropped with it and the text between stays.
    let mut dropped: Vec<bool> = Vec::new();
    let parser = Parser::new_ext(body, options).filter_map(move |event| {
        let start_kept = match &event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(link_destination_allowed(dest_url)),
            Event::Start(Tag::Image { dest_url, .. }) => Some(image_destination_allowed(dest_url)),
            _ => None,
        };
        if let Some(kept) = start_kept {
            dropped.push(!kept);
            return kept.then_some(event);
        }
        Some(match event {
            Event::End(TagEnd::Link | TagEnd::Image) => {
                return (!dropped.pop().unwrap_or(false)).then_some(event);
            }
            // The one exception to "raw HTML is escaped": an inline event that
            // is exactly a bare `<br>`. An `Event::Html` block, even one that
            // begins with `<br>`, runs to the next blank line and can carry
            // anything after it, so it stays escaped.
            Event::InlineHtml(s) if is_bare_br(&s) => Event::HardBreak,
            // Escape all other raw HTML so document-controlled scripts cannot
            // execute.
            Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
            // Render math as readable LaTeX source (RFC-038).
            // A progressive-enhancement KaTeX pass can be layered on top later.
            Event::InlineMath(code) => Event::Html(
                format!("<code class=\"math-inline\">{}</code>", html_escape(&code)).into(),
            ),
            Event::DisplayMath(code) => Event::Html(
                format!(
                    "<pre class=\"math-block\"><code>{}</code></pre>\n",
                    html_escape(&code)
                )
                .into(),
            ),
            other => other,
        })
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
