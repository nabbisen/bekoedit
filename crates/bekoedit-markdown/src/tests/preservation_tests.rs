//! Golden source-preservation cases (RFC-000 §13).
//!
//! The invariant under test: editing one block leaves every other byte of
//! the document untouched, for documents containing the full adversarial
//! mix — multibyte text, CRLF, front matter, HTML, mixed fence and list
//! marker styles, non-1 ordered lists, reference links, and blank-line
//! sensitive regions.

use crate::form::{FormBlockEdit, FormEditCommand, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

const GOLDEN: &str = concat!(
    "---\n",
    "title: ゴールデン\n",
    "tags: [a, b]\n",
    "---\n",
    "\n",
    "# 見出し 🎌\n",
    "\n",
    "Paragraph with [ref link][r1] and `code`.\n",
    "\n",
    "* star item\n",
    "+ plus item\n",
    "\n",
    "7. seven\n",
    "8. eight\n",
    "\n",
    "> simple quote\n",
    "\n",
    "~~~~text\n",
    "fenced with tildes\n",
    "~~~~\n",
    "\n",
    "<div class=\"x\">\n",
    "raw html\n",
    "</div>\n",
    "\n",
    "| t | u |\n",
    "|---|---|\n",
    "| 1 | 2 |\n",
    "\n",
    "[r1]: https://example.com\n",
);

/// Asserts that applying `edit` to block `ordinal` changes only the bytes
/// the edit semantically targets: the document must equal the original
/// with exactly one region replaced.
fn assert_only_region_changed(doc: &str, ordinal: usize, edit: FormBlockEdit) -> String {
    let idx = MarkdownIndex::build(doc, 1);
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[ordinal].block_id,
        client_block_fingerprint: Some(idx.blocks[ordinal].block_id.fingerprint),
        edit,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).expect("resolve");
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).expect("apply");
    // Prefix and suffix outside the patch range are byte-identical.
    assert_eq!(&out[..patch.range.start], &doc[..patch.range.start]);
    let new_suffix_start = patch.range.start + patch.replacement.len();
    assert_eq!(&out[new_suffix_start..], &doc[patch.range.end..]);
    out
}

#[test]
fn editing_heading_preserves_everything_else() {
    let out = assert_only_region_changed(
        GOLDEN,
        1,
        FormBlockEdit::ReplacePlainText {
            text: "新見出し".into(),
        },
    );
    assert!(out.contains("# 新見出し"));
    assert!(out.contains("title: ゴールデン"), "front matter untouched");
    assert!(out.contains("~~~~text"), "tilde fence untouched");
    assert!(
        out.contains("[r1]: https://example.com"),
        "ref link def untouched"
    );
}

#[test]
fn editing_paragraph_preserves_mixed_list_markers() {
    let out = assert_only_region_changed(
        GOLDEN,
        2,
        FormBlockEdit::ReplacePlainText {
            text: "Plain replacement.".into(),
        },
    );
    assert!(out.contains("* star item"));
    assert!(out.contains("+ plus item"));
    assert!(out.contains("7. seven"));
}

#[test]
fn editing_code_preserves_html_and_table_islands() {
    // Blocks: 0 front matter, 1 heading, 2 paragraph, 3 star list,
    // 4 plus list, 5 ordered list, 6 quote, 7 code, 8 html, 9 table, 10 ref def.
    let out = assert_only_region_changed(
        GOLDEN,
        7,
        FormBlockEdit::ReplaceCodeBlock {
            language: None,
            code: "changed\n".into(),
        },
    );
    assert!(out.contains("~~~~\nchanged\n~~~~"), "tilde style preserved");
    assert!(out.contains("<div class=\"x\">"));
    assert!(out.contains("| t | u |"));
}

#[test]
fn crlf_document_edit_preserves_crlf() {
    let doc = "# Title\r\n\r\nold\r\n\r\n- a\r\n";
    let idx = MarkdownIndex::build(doc, 1);
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[1].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText { text: "new".into() },
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).unwrap();
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    assert_eq!(out, "# Title\r\n\r\nnew\r\n\r\n- a\r\n");
}

#[test]
fn full_document_roundtrip_without_edits_is_identity() {
    // Building projections must never mutate the source (projection
    // invariant): the index and form projection are read-only views.
    let idx = MarkdownIndex::build(GOLDEN, 1);
    let _projection = crate::form::FormProjection::build(GOLDEN, &idx);
    // Reconstruct the document from block ranges + gaps and verify identity.
    let mut reconstructed = String::new();
    let mut cursor = 0usize;
    for block in &idx.blocks {
        reconstructed.push_str(&GOLDEN[cursor..block.source_range.start]);
        reconstructed.push_str(&GOLDEN[block.source_range.start..block.source_range.end]);
        cursor = block.source_range.end;
    }
    reconstructed.push_str(&GOLDEN[cursor..]);
    assert_eq!(reconstructed, GOLDEN);
}

#[test]
fn blank_line_sensitive_document_survives_item_edit() {
    let doc = "para one\n\n\n- a\n- b\n\n\npara two\n";
    let idx = MarkdownIndex::build(doc, 1);
    let list_ordinal = idx
        .blocks
        .iter()
        .position(|b| !b.items.is_empty())
        .expect("list");
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[list_ordinal].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplaceListItemText {
            item_ordinal: 0,
            text: "alpha".into(),
        },
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).unwrap();
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    assert_eq!(out, "para one\n\n\n- alpha\n- b\n\n\npara two\n");
}
