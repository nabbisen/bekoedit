//! Task 030: Preview's link and image destination allowlist, and the bare
//! inline `<br>` exception. Every row of the policy table, the evasions a
//! deny-list would miss, and each accepted and rejected `<br>` spelling.

use crate::preview::render_preview_html;

fn render(markdown: &str) -> String {
    render_preview_html(markdown)
}

/// The link's text survives with no `<a`; nothing live carries the scheme.
fn assert_link_dropped(markdown: &str, text: &str) {
    let html = render(markdown);
    assert_eq!(html, format!("<p>{text}</p>\n"), "{markdown:?}");
}

fn assert_link_kept(markdown: &str, href: &str) {
    let html = render(markdown);
    assert_eq!(
        html,
        format!("<p><a href=\"{href}\">x</a></p>\n"),
        "{markdown:?}"
    );
}

fn assert_image_dropped(markdown: &str) {
    let html = render(markdown);
    assert_eq!(
        html, "<p>alt</p>\n",
        "the alt text stays, no <img>: {markdown:?}"
    );
}

fn assert_image_kept(markdown: &str, src: &str) {
    let html = render(markdown);
    assert_eq!(
        html,
        format!("<p><img src=\"{src}\" alt=\"alt\" /></p>\n"),
        "{markdown:?}"
    );
}

// ---- the table, links ----------------------------------------------------

#[test]
fn links_to_http_https_mailto_relative_and_fragment_are_allowed() {
    assert_link_kept("[x](http://a.example/p)", "http://a.example/p");
    assert_link_kept(
        "[x](https://a.example/p?q=1#f)",
        "https://a.example/p?q=1#f",
    );
    assert_link_kept("[x](mailto:me@a.example)", "mailto:me@a.example");
    assert_link_kept("[x](notes/other.md)", "notes/other.md");
    assert_link_kept("[x](./other.md)", "./other.md");
    assert_link_kept("[x](#section)", "#section");
}

#[test]
fn links_with_any_other_scheme_are_dropped_keeping_their_text() {
    for destination in [
        "javascript:alert(1)",
        "vbscript:msgbox(1)",
        "file:///etc/passwd",
        "data:text/html,alert(1)",
        "data:image/svg+xml,svgdata",
        "data:image/png;base64,AAAA",
        "ftp://a.example/f",
        "tel:+1555",
    ] {
        assert_link_dropped(&format!("[x](<{destination}>)"), "x");
    }
}

#[test]
fn a_dropped_link_keeps_its_inline_content() {
    assert_link_dropped(
        "[*em* and `code`](javascript:x)",
        "<em>em</em> and <code>code</code>",
    );
}

// ---- the table, images ---------------------------------------------------

#[test]
fn images_from_http_https_relative_and_fragment_are_allowed() {
    assert_image_kept("![alt](http://a.example/i.png)", "http://a.example/i.png");
    assert_image_kept("![alt](https://a.example/i.png)", "https://a.example/i.png");
    assert_image_kept("![alt](img/i.png)", "img/i.png");
    assert_image_kept("![alt](#frag)", "#frag");
}

#[test]
fn data_images_of_the_four_raster_types_are_allowed() {
    for kind in ["png", "jpeg", "gif", "webp"] {
        let destination = format!("data:image/{kind};base64,AAAA");
        assert_image_kept(&format!("![alt]({destination})"), &destination);
    }
    assert_image_kept("![alt](data:image/png,AAAA)", "data:image/png,AAAA");
}

#[test]
fn mailto_and_every_other_image_source_is_dropped_keeping_the_alt_text() {
    for destination in [
        "mailto:me@a.example",
        "javascript:alert(1)",
        "vbscript:x",
        "file:///etc/passwd",
        "data:text/html,alert(1)",
        "data:image/svg+xml;base64,AAAA",
        "data:image/svg+xml,svgdata",
        "data:application/octet-stream,AAAA",
    ] {
        assert_image_dropped(&format!("![alt](<{destination}>)"));
    }
}

// ---- evasions ------------------------------------------------------------

#[test]
fn scheme_case_does_not_matter_in_either_direction() {
    assert_link_dropped("[x](JaVaScRiPt:alert(1))", "x");
    assert_link_dropped("[x](JAVASCRIPT:alert(1))", "x");
    assert_link_kept("[x](HTTPS://a.example/)", "HTTPS://a.example/");
    assert_link_kept("[x](MailTo:me@a.example)", "MailTo:me@a.example");
    assert_image_kept(
        "![alt](DATA:IMAGE/PNG;base64,AAAA)",
        "DATA:IMAGE/PNG;base64,AAAA",
    );
    assert_image_dropped("![alt](<DATA:IMAGE/SVG+XML,svgdata>)");
}

#[test]
fn leading_whitespace_and_control_characters_do_not_hide_a_scheme() {
    assert_link_dropped("[x](<\u{1}javascript:alert(1)>)", "x");
    assert_link_dropped("[x](<\u{1f}\u{7f}javascript:alert(1)>)", "x");
    assert_link_dropped("[x](< javascript:alert(1)>)", "x");
    assert_link_dropped("[x](<\tjavascript:alert(1)>)", "x");
    assert_image_dropped("![alt](<\u{1}javascript:alert(1)>)");
}

#[test]
fn a_tab_inside_a_scheme_does_not_hide_it() {
    // A browser's URL parser removes it, so this is `javascript:` to it. (A
    // CR or LF cannot occur inside a Markdown link destination at all.)
    assert_link_dropped("[x](<java\tscript:alert(1)>)", "x");
    assert_image_dropped("![alt](<java\tscript:alert(1)>)");
}

#[test]
fn entity_encoded_schemes_are_caught_after_decoding() {
    assert_link_dropped("[x](&#106;avascript:alert(1))", "x");
    assert_link_dropped("[x](&#x6A;avascript:alert(1))", "x");
    assert_link_dropped("[x](&#74;&#97;vaScript:alert(1))", "x");
    assert_image_dropped("![alt](&#106;avascript:alert(1))");
    // Whatever an entity decodes to, nothing live carries the scheme.
    let html = render("[x](javascript&colon;alert(1))").to_ascii_lowercase();
    assert!(!html.contains("javascript"), "{html}");
}

#[test]
fn javascript_inside_an_image_is_dropped_even_within_an_allowed_link() {
    assert_eq!(
        render("[![alt](javascript:x)](https://a.example/)"),
        "<p><a href=\"https://a.example/\">alt</a></p>\n"
    );
}

#[test]
fn an_allowed_image_inside_a_dropped_link_stays() {
    assert_eq!(
        render("[![alt](https://a.example/i.png)](javascript:x)"),
        "<p><img src=\"https://a.example/i.png\" alt=\"alt\" /></p>\n"
    );
}

#[test]
fn reference_style_and_autolink_forms_go_through_the_same_rule() {
    assert_link_dropped("[x][r]\n\n[r]: javascript:alert(1)", "x");
    assert_eq!(
        render("<javascript:alert(1)>"),
        "<p>javascript:alert(1)</p>\n",
        "an autolink to a dropped scheme keeps its visible text"
    );
    assert_eq!(
        render("<https://a.example>"),
        "<p><a href=\"https://a.example\">https://a.example</a></p>\n"
    );
    assert_eq!(
        render("<me@a.example>"),
        "<p><a href=\"mailto:me@a.example\">me@a.example</a></p>\n"
    );
}

// ---- Part B: a bare inline <br> -----------------------------------------

#[test]
fn a_bare_inline_br_is_a_line_break_in_each_accepted_spelling() {
    for spelling in ["<br>", "<br/>", "<br />", "<BR>", "<Br />", "<bR/>"] {
        assert_eq!(
            render(&format!("a{spelling}b")),
            "<p>a<br />\nb</p>\n",
            "{spelling}"
        );
    }
}

#[test]
fn a_br_in_a_table_cell_is_a_line_break() {
    let html = render("| a | b |\n|---|---|\n| x<br>y | z<br/>w |\n");
    assert!(
        html.contains("<td>x<br />\ny</td>") && html.contains("<td>z<br />\nw</td>"),
        "{html}"
    );
}

#[test]
fn a_br_with_any_attribute_stays_escaped() {
    assert_eq!(
        render("a <br class=\"x\"> b"),
        "<p>a &lt;br class=\"x\"&gt; b</p>\n"
    );
    assert_eq!(
        render("a <br onclick=alert(1)> b"),
        "<p>a &lt;br onclick=alert(1)&gt; b</p>\n"
    );
}

#[test]
fn a_malformed_br_stays_escaped() {
    // Not inline HTML at all: it reaches the renderer as text and is escaped.
    assert_eq!(render("a <BR/ > b"), "<p>a &lt;BR/ &gt; b</p>\n");
    assert_eq!(render("a <br//> b"), "<p>a &lt;br//&gt; b</p>\n");
    assert_eq!(render("a <brx> b"), "<p>a &lt;brx&gt; b</p>\n");
}

#[test]
fn a_br_followed_by_text_is_a_break_then_that_text() {
    // `<br>x` reaches the renderer as an inline `<br>` and a separate text
    // event `x`, so the tag itself is exactly a bare `<br>`.
    assert_eq!(render("<br>x"), "<p><br />\nx</p>\n");
}

#[test]
fn an_html_block_stays_escaped_even_when_it_is_just_a_br() {
    assert_eq!(render("<br>"), "&lt;br&gt;");
    assert_eq!(render("<br/>"), "&lt;br/&gt;");
    assert_eq!(render("<br />"), "&lt;br /&gt;");
    // A block runs to the next blank line and can carry anything after it.
    assert_eq!(
        render("<br>\n<script>alert(1)</script>"),
        "&lt;br&gt;\n&lt;script&gt;alert(1)&lt;/script&gt;"
    );
}

#[test]
fn other_inline_html_stays_escaped_beside_a_bare_br() {
    assert_eq!(
        render("a<br><script>alert(1)</script>b"),
        "<p>a<br />\n&lt;script&gt;alert(1)&lt;/script&gt;b</p>\n"
    );
}
