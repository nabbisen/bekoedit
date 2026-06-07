//! RFC-016 / RFC-018 acceptance criteria: semantic edits patch only the
//! targeted region, stale commands are rejected, and JS cannot force
//! arbitrary mutations.

use crate::form::{FormBlockEdit, FormEditCommand, FormEditError, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

/// Applies a form edit end-to-end the way the document session does:
/// resolve -> patch -> reparse.
fn apply(text: &str, ordinal: usize, edit: FormBlockEdit) -> Result<String, FormEditError> {
    let idx = MarkdownIndex::build(text, 1);
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: idx.blocks[ordinal].block_id,
        client_block_fingerprint: None,
        edit,
    };
    let patch = resolve_form_edit(text, &idx, &cmd)?;
    let mut out = text.to_string();
    apply_patch(&mut out, 1, &patch).expect("patch applies");
    Ok(out)
}

#[test]
fn paragraph_text_replacement_keeps_surroundings() {
    let doc = "# Title\n\nold text\n\n- a\n- b\n";
    let out = apply(
        doc,
        1,
        FormBlockEdit::ReplacePlainText {
            text: "new text".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "# Title\n\nnew text\n\n- a\n- b\n");
}

#[test]
fn heading_text_replacement_preserves_marker() {
    let doc = "## Goals ##\n\nbody\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ReplacePlainText {
            text: "Aims".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "## Aims ##\n\nbody\n");
}

#[test]
fn set_heading_level_rewrites_only_the_marker_run() {
    let doc = "## Goals\n\nbody\n";
    let out = apply(doc, 0, FormBlockEdit::SetHeadingLevel { level: 3 }).unwrap();
    assert_eq!(out, "### Goals\n\nbody\n");
}

#[test]
fn setext_heading_level_change_is_rejected() {
    let doc = "Title\n=====\n\nbody\n";
    let err = apply(doc, 0, FormBlockEdit::SetHeadingLevel { level: 2 }).unwrap_err();
    assert!(matches!(
        err,
        FormEditError::UnsupportedEditOperation { .. }
    ));
}

#[test]
fn task_toggle_patches_exactly_one_byte() {
    let doc = "- [ ] write tests\n- [x] design\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ToggleTaskChecked {
            item_ordinal: 0,
            checked: true,
        },
    )
    .unwrap();
    assert_eq!(out, "- [x] write tests\n- [x] design\n");
    let out2 = apply(
        doc,
        0,
        FormBlockEdit::ToggleTaskChecked {
            item_ordinal: 1,
            checked: false,
        },
    )
    .unwrap();
    assert_eq!(out2, "- [ ] write tests\n- [ ] design\n");
}

#[test]
fn list_item_text_preserves_marker_styles() {
    let doc = "* alpha\n* beta\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ReplaceListItemText {
            item_ordinal: 1,
            text: "gamma".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "* alpha\n* gamma\n", "asterisk markers must survive");
}

#[test]
fn ordered_list_numbering_style_is_untouched() {
    let doc = "3. third\n4. fourth\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ReplaceListItemText {
            item_ordinal: 0,
            text: "tres".into(),
        },
    )
    .unwrap();
    assert_eq!(
        out, "3. tres\n4. fourth\n",
        "non-1 start numbering preserved"
    );
}

#[test]
fn code_replacement_preserves_tilde_fence_style() {
    let doc = "intro\n\n~~~~js\nconsole.log(1)\n~~~~\n";
    let out = apply(
        doc,
        1,
        FormBlockEdit::ReplaceCodeBlock {
            language: Some("ts".into()),
            code: "console.log(2)\n".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "intro\n\n~~~~ts\nconsole.log(2)\n~~~~\n");
}

#[test]
fn code_fence_lengthens_when_code_collides() {
    let doc = "```\nx\n```\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ReplaceCodeBlock {
            language: None,
            code: "```inner\n".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "````\n```inner\n````\n");
}

#[test]
fn raw_island_edit_patches_only_the_island() {
    let doc = "---\ntitle: a\n---\n\nbody\n";
    let out = apply(
        doc,
        0,
        FormBlockEdit::ReplaceRawIsland {
            text: "---\ntitle: b\n---".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "---\ntitle: b\n---\n\nbody\n");
}

#[test]
fn structured_edit_on_island_is_rejected() {
    let doc = "<div>\nhtml\n</div>\n";
    let err = apply(doc, 0, FormBlockEdit::ReplacePlainText { text: "x".into() }).unwrap_err();
    assert!(matches!(
        err,
        FormEditError::UnsupportedEditOperation { .. }
    ));
}

#[test]
fn delete_block_removes_trailing_blank_lines() {
    let doc = "# A\n\nmiddle\n\nend\n";
    let out = apply(doc, 1, FormBlockEdit::DeleteBlock).unwrap();
    assert_eq!(out, "# A\n\nend\n");
}

#[test]
fn delete_horizontal_rule() {
    let doc = "a\n\n---\n\nb\n";
    let out = apply(doc, 1, FormBlockEdit::DeleteBlock).unwrap();
    assert_eq!(out, "a\n\nb\n");
}

#[test]
fn stale_revision_command_is_rejected() {
    let doc = "para\n";
    let idx = MarkdownIndex::build(doc, 5);
    let cmd = FormEditCommand {
        base_revision: 4,
        block_id: idx.blocks[0].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText { text: "x".into() },
    };
    let err = resolve_form_edit(doc, &idx, &cmd).unwrap_err();
    assert_eq!(
        err,
        FormEditError::DocumentRevisionMismatch {
            base: 4,
            current: 5
        }
    );
}

#[test]
fn fingerprint_mismatch_is_rejected() {
    let doc = "para\n";
    let idx = MarkdownIndex::build(doc, 1);
    let mut id = idx.blocks[0].block_id;
    id.fingerprint.content_hash ^= 0xdead;
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText { text: "x".into() },
    };
    assert_eq!(
        resolve_form_edit(doc, &idx, &cmd).unwrap_err(),
        FormEditError::BlockNotFound
    );
}

#[test]
fn multibyte_form_edit_is_utf8_safe() {
    let doc = "# 見出し\n\n日本語の段落。\n";
    let out = apply(
        doc,
        1,
        FormBlockEdit::ReplacePlainText {
            text: "新しい段落😀".into(),
        },
    )
    .unwrap();
    assert_eq!(out, "# 見出し\n\n新しい段落😀\n");
}

#[test]
fn form_projection_marks_islands_and_editables() {
    use crate::form::{FormBlockDisplay, FormProjection};
    let doc = "# T\n\npara\n\n| a |\n|---|\n";
    let idx = MarkdownIndex::build(doc, 1);
    let proj = FormProjection::build(doc, &idx);
    assert_eq!(proj.document_revision, 1);
    assert!(matches!(
        proj.blocks[0].display,
        FormBlockDisplay::Heading { level: 1, .. }
    ));
    assert!(matches!(
        proj.blocks[1].display,
        FormBlockDisplay::Paragraph { .. }
    ));
    assert!(matches!(
        proj.blocks[2].display,
        FormBlockDisplay::RawIsland { .. }
    ));
}
