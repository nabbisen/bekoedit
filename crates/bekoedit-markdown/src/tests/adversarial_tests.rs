// Adversarial golden document — source preservation (MVP acceptance checklist).
//
// One document containing every tricky Markdown pattern from the checklist:
// CRLF line endings, Japanese + emoji, mixed list markers, tilde fences,
// non-1 ordered lists, reference links, front matter, raw HTML, and tables.
//
// Each test edits exactly one block and asserts that every byte outside
// that block's source range is identical to the original.

use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

/// The adversarial document.  Uses CRLF throughout to exercise line-ending
/// preservation on Windows-authored files.
fn adversarial_doc() -> String {
    // Note: \r\n line endings throughout.
    [
        "---\r\n",
        "title: Adversarial\r\n",
        "tags: [test, \"edge case\"]\r\n",
        "---\r\n",
        "\r\n",
        "# こんにちは世界 😀\r\n",
        "\r\n",
        "Paragraph with **bold**, _italic_, and `code`.\r\n",
        "\r\n",
        "* item A\r\n",
        "+ item B\r\n",
        "- item C\r\n",
        "\r\n",
        "3. third\r\n",
        "4. fourth\r\n",
        "\r\n",
        "~~~~rust\r\n",
        "fn hello() { println!(\"world\"); }\r\n",
        "~~~~\r\n",
        "\r\n",
        "> blockquote line\r\n",
        "\r\n",
        "See [reference link][ref].\r\n",
        "\r\n",
        "<div class=\"custom\">\r\n",
        "raw HTML block\r\n",
        "</div>\r\n",
        "\r\n",
        "| Name  | Score |\r\n",
        "|-------|-------|\r\n",
        "| Alice | 42    |\r\n",
        "\r\n",
        "Another paragraph after the table.\r\n",
        "\r\n",
        "[ref]: https://example.com\r\n",
    ]
    .concat()
}

/// Edits the given block (by ordinal in the block list) and verifies that
/// all bytes outside the edited range are unchanged.
fn edit_one_block_check_rest(doc: &str, block_ordinal: usize, edit: FormBlockEdit) {
    let idx = MarkdownIndex::build(doc, 1);
    let block = &idx.blocks[block_ordinal];

    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: block.block_id,
        client_block_fingerprint: None,
        edit,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd)
        .unwrap_or_else(|e| panic!("resolve failed for block {block_ordinal}: {e}"));

    let mut result = doc.to_string();
    apply_patch(&mut result, 1, &patch).unwrap_or_else(|e| panic!("patch failed: {e}"));

    // Everything before the edited range must be byte-identical.
    assert_eq!(
        &result[..patch.range.start],
        &doc[..patch.range.start],
        "bytes BEFORE edit changed (block {block_ordinal})"
    );
    // Everything after the edited range must be byte-identical.
    let after_doc = patch.range.end;
    let after_result = patch.range.start + patch.replacement.len();
    assert_eq!(
        &result[after_result..],
        &doc[after_doc..],
        "bytes AFTER edit changed (block {block_ordinal})"
    );
    // The edited region must differ (otherwise the test isn't exercising anything).
    let new_content = &result[patch.range.start..after_result];
    let old_content = &doc[patch.range.start..patch.range.end];
    assert_ne!(
        new_content, old_content,
        "block {block_ordinal} text didn't change — edit may have had no effect"
    );
}

#[test]
fn adversarial_edit_heading_only_changes_heading() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    let heading_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::Heading)
        .expect("heading block");
    edit_one_block_check_rest(
        &doc,
        heading_ord,
        FormBlockEdit::ReplacePlainText {
            text: "Hello World 🌍".into(),
        },
    );
}

#[test]
fn adversarial_edit_paragraph_only_changes_paragraph() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    let para_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::Paragraph)
        .expect("paragraph block");
    edit_one_block_check_rest(
        &doc,
        para_ord,
        FormBlockEdit::ReplacePlainText {
            text: "Changed paragraph text.".into(),
        },
    );
}

#[test]
fn adversarial_tilde_fence_preserves_fence_style_and_crlf() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    let code_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::FencedCode)
        .expect("fenced code block");
    edit_one_block_check_rest(
        &doc,
        code_ord,
        FormBlockEdit::ReplaceCodeBlock {
            language: Some("ts".into()),
            code: "const x = 1;\r\n".into(),
        },
    );
    // Verify the tilde fence is preserved by applying the patch and checking
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[code_ord].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplaceCodeBlock {
            language: Some("ts".into()),
            code: "const x = 1;\r\n".into(),
        },
    };
    let patch = resolve_form_edit(&doc, &idx, &cmd).unwrap();
    assert!(
        patch.replacement.contains("~~~~"),
        "tilde fence not preserved"
    );
}

#[test]
fn adversarial_ordered_list_preserves_non_1_start() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    let list_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::OrderedList)
        .expect("ordered list");
    edit_one_block_check_rest(
        &doc,
        list_ord,
        FormBlockEdit::ReplaceListItemText {
            item_ordinal: 0,
            text: "tres".into(),
        },
    );
    // The patch only replaces the item content; the "3." marker stays
    // unchanged in the surrounding source.  Verify the full result contains "3. tres".
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[list_ord].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplaceListItemText {
            item_ordinal: 0,
            text: "tres".into(),
        },
    };
    let patch = resolve_form_edit(&doc, &idx, &cmd).unwrap();
    let mut result = doc.clone();
    apply_patch(&mut result, 1, &patch).unwrap();
    assert!(
        result.contains("3. tres"),
        "non-1 list start not preserved; got: {result:?}"
    );
}

#[test]
fn adversarial_front_matter_preserved_as_island() {
    use crate::island::RawIslandType;
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::FrontMatter),
        "front matter not classified as island"
    );
}

#[test]
fn adversarial_html_block_preserved_as_island() {
    use crate::island::RawIslandType;
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::HtmlBlock),
        "HTML block not classified as island"
    );
}

#[test]
fn adversarial_crlf_preserved_throughout() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    // Edit paragraph and verify CRLF still appears in result
    let para_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::Paragraph)
        .expect("paragraph");
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[para_ord].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: "new content".into(),
        },
    };
    let patch = resolve_form_edit(&doc, &idx, &cmd).unwrap();
    let mut result = doc.clone();
    apply_patch(&mut result, 1, &patch).unwrap();
    // The document outside the edit must retain CRLF
    let before = &result[..patch.range.start];
    assert!(before.contains("\r\n"), "CRLF not preserved before edit");
    let after = &result[patch.range.start + patch.replacement.len()..];
    assert!(after.contains("\r\n"), "CRLF not preserved after edit");
}

#[test]
fn adversarial_japanese_emoji_in_heading_survives_patch_safely() {
    let doc = adversarial_doc();
    let idx = MarkdownIndex::build(&doc, 1);
    let heading_ord = idx
        .blocks
        .iter()
        .position(|b| b.kind == BlockKind::Heading)
        .expect("heading");
    // The heading contains "こんにちは世界 😀" — verify we can read it back
    let heading = &idx.headings[0];
    assert!(
        heading.text.contains("世界"),
        "Japanese text not in heading"
    );
    assert!(heading.text.contains("😀"), "Emoji not in heading");
    // Edit and confirm source range validity (no panic = correct UTF-8 handling)
    edit_one_block_check_rest(
        &doc,
        heading_ord,
        FormBlockEdit::ReplacePlainText {
            text: "新しい見出し".into(),
        },
    );
}
