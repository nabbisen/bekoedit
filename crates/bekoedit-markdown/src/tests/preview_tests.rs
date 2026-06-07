//! RFC-012 acceptance criteria: preview renders from canonical text and
//! raw HTML is displayed according to the safety policy (never executed).

use crate::preview::render_preview_html;

#[test]
fn renders_basic_markdown() {
    let html = render_preview_html("# Title\n\nSome *emphasis*.\n");
    assert!(html.contains("<h1>Title</h1>"));
    assert!(html.contains("<em>emphasis</em>"));
}

#[test]
fn script_blocks_are_escaped_not_executed() {
    let html = render_preview_html("<script>alert('x')</script>\n");
    assert!(
        !html.contains("<script>"),
        "script tag must not survive: {html}"
    );
    assert!(
        html.contains("&lt;script&gt;"),
        "script must be shown escaped: {html}"
    );
}

#[test]
fn inline_html_is_escaped() {
    let html = render_preview_html("text with <b onclick=\"evil()\">bold</b> inline\n");
    assert!(!html.contains("<b onclick"));
    assert!(html.contains("&lt;b onclick"));
}

#[test]
fn front_matter_is_not_rendered_as_content() {
    let html = render_preview_html("---\ntitle: hidden\n---\n\n# Visible\n");
    assert!(!html.contains("hidden"));
    assert!(html.contains("<h1>Visible</h1>"));
}

#[test]
fn task_lists_render_as_checkboxes() {
    let html = render_preview_html("- [x] done\n- [ ] todo\n");
    assert!(html.contains("checkbox"));
}
