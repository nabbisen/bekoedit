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
