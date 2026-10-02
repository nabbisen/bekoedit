// RFC-030 inline formatting toggle tests.

use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, FormEditError, InlineFormat, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn apply_inline(doc: &str, edit: FormBlockEdit) -> Result<String, FormEditError> {
    let idx = MarkdownIndex::build(doc, 1);
    let para = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .unwrap();
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: para.block_id,
        client_block_fingerprint: None,
        edit,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd)?;
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    Ok(out)
}

#[test]
fn toggle_bold_wraps_selection() {
    let doc = "# T\n\nHello world\n";
    let out = apply_inline(
        doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Bold,
            utf16_start: 6,
            utf16_len: 5,
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(out, "# T\n\nHello **world**\n");
}

#[test]
fn toggle_bold_unwraps_existing_markers() {
    let doc = "# T\n\nHello **world**\n";
    let out = apply_inline(
        doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Bold,
            utf16_start: 6,
            utf16_len: 9,
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(out, "# T\n\nHello world\n");
}

#[test]
fn toggle_italic_wraps() {
    let doc = "# T\n\nsome text\n";
    let out = apply_inline(
        doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Italic,
            utf16_start: 5,
            utf16_len: 4,
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(out, "# T\n\nsome _text_\n");
}

#[test]
fn toggle_link_wraps_with_url() {
    let doc = "# T\n\nClick here\n";
    let out = apply_inline(
        doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Link,
            utf16_start: 6,
            utf16_len: 4,
            link_url: Some("https://example.com".into()),
        },
    )
    .unwrap();
    assert_eq!(out, "# T\n\nClick [here](https://example.com)\n");
}

// ---- task 047 Part C: a code span's fence is never broken by a backtick
// already inside the selection ----

fn assert_single_code_event(markdown: &str, expected_text: &str) {
    let events: Vec<String> = pulldown_cmark::Parser::new(markdown)
        .filter_map(|event| match event {
            pulldown_cmark::Event::Code(text) => Some(text.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        events,
        vec![expected_text.to_string()],
        "parsing {markdown:?}"
    );
}

/// Wraps `original`, checks the exact fence `wrap_code` must produce and
/// that `pulldown-cmark` parses it back to one `Code` event holding
/// `original` verbatim, then unwraps it again and checks the bytes are
/// exactly the original document -- "unwrap recognises what it wrapped"
/// (task 047 §3).
fn code_round_trip(original: &str, expected_wrapped: &str) {
    let doc = format!("# T\n\n{original} end\n");
    let wrapped_doc = apply_inline(
        &doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Code,
            utf16_start: 0,
            utf16_len: original.encode_utf16().count(),
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(wrapped_doc, format!("# T\n\n{expected_wrapped} end\n"));
    assert_single_code_event(&wrapped_doc, original);

    let back = apply_inline(
        &wrapped_doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Code,
            utf16_start: 0,
            utf16_len: expected_wrapped.encode_utf16().count(),
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(back, doc);
}

#[test]
fn code_with_no_backtick_uses_a_single_backtick_fence() {
    code_round_trip("ab", "`ab`");
}

#[test]
fn code_around_one_contained_backtick_uses_a_double_backtick_fence() {
    code_round_trip("a`b", "``a`b``");
}

#[test]
fn code_around_a_contained_double_backtick_uses_a_triple_backtick_fence() {
    code_round_trip("a``b", "```a``b```");
}

#[test]
fn code_around_a_leading_backtick_is_padded_with_spaces() {
    code_round_trip("`x", "`` `x ``");
}

#[test]
fn inline_format_multibyte_utf16() {
    // "世界" starts at UTF-16 offset 5 (こんにちは = 5 × 1 UTF-16 unit each)
    let doc = "# T\n\nこんにちは世界\n";
    let out = apply_inline(
        doc,
        FormBlockEdit::ToggleInline {
            kind: InlineFormat::Italic,
            utf16_start: 5,
            utf16_len: 2,
            link_url: None,
        },
    )
    .unwrap();
    assert_eq!(out, "# T\n\nこんにちは_世界_\n");
}
