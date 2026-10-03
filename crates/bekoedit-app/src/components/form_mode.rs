//! Form Mode surface (RFC-016/017/018/027/028/030).
//!
//! Renders each block from `FormProjection` as a typed form control.
//! New in v0.4.0:
//! - **Inline formatting toolbar** (RFC-030): B / I / ` / 🔗 buttons above
//!   paragraph/heading/blockquote fields; uses `onmousedown preventDefault`
//!   to keep the textarea focus, then reads `selectionStart/End` via eval.
//! - **Table grid** (RFC-027): simple GFM tables as editable cell grids.
//! - **Image card** (RFC-028): image preview + editable alt/src fields.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{
    FormBlockDisplay, FormBlockEdit, FormEditCommand, FormProjection, fingerprint::BlockId,
};

use crate::i18n::Lang;
use crate::state::now_ms;

#[component]
pub fn FormMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let projection: Option<FormProjection> =
        state.read().session.as_ref().map(|s| s.form_projection());
    let Some(projection) = projection else {
        return rsx! { div {} };
    };
    let revision = projection.document_revision;

    rsx! {
        div { class: "form-mode", "data-source-focus-launch-region": "form",
            for block in projection.blocks {
                FormBlockView {
                    key: "{block.block_id.ordinal}-{block.block_id.fingerprint.content_hash}",
                    block_id: block.block_id,
                    display: block.display.clone(),
                    revision,
                    lang,
                }
            }
        }
    }
}

fn dispatch(mut state: Signal<AppState>, revision: u64, block_id: BlockId, edit: FormBlockEdit) {
    // Task 051 §2.1: `shortcuts.js` dispatches a plain `change` event on
    // a focused Form field before relaying every shortcut, whether or
    // not the user actually typed anything since it was focused.
    // `edit_form` calls `after_edit` unconditionally on success
    // (`store.rs`) -- dirtying the document, scheduling an autosave, and
    // writing a recovery snapshot even when nothing actually changed --
    // so a `ReplacePlainText`/`ReplaceTableCell` edit that just repeats
    // what is already committed must never reach it.
    if let Some(projection) = state.read().session.as_ref().map(|s| s.form_projection())
        && is_unchanged_text_edit(&projection, block_id, &edit)
    {
        return;
    }
    let cmd = FormEditCommand {
        base_revision: revision,
        block_id,
        client_block_fingerprint: Some(block_id.fingerprint),
        edit,
    };
    let _ = state.write().edit_form(&cmd, now_ms());
}

/// Whether `edit` would replace a `ReplacePlainText`/`ReplaceTableCell`
/// block's text with exactly what `projection` already has committed
/// there -- the one check [`dispatch`] needs before deciding whether to
/// skip the edit entirely (task 051 §2.1). Every other edit kind is
/// never skipped.
fn is_unchanged_text_edit(
    projection: &FormProjection,
    block_id: BlockId,
    edit: &FormBlockEdit,
) -> bool {
    let (cell, new_text) = match edit {
        FormBlockEdit::ReplacePlainText { text } => (None, text.as_str()),
        FormBlockEdit::ReplaceTableCell { row, col, text } => (Some((*row, *col)), text.as_str()),
        _ => return false,
    };
    let Some(block) = projection.blocks.iter().find(|b| b.block_id == block_id) else {
        return false;
    };
    current_text(&block.display, cell) == Some(new_text)
}

/// The current text at `cell` (or the whole block, if `cell` is `None`).
/// `None` if `display`/`cell` do not agree on what kind of field this
/// is (a cell address against a non-table block, or one the table does
/// not have).
fn current_text(display: &FormBlockDisplay, cell: Option<(usize, usize)>) -> Option<&str> {
    match (display, cell) {
        (FormBlockDisplay::Paragraph { text } | FormBlockDisplay::Blockquote { text }, None) => {
            Some(text)
        }
        (FormBlockDisplay::Heading { text, .. }, None) => Some(text),
        (FormBlockDisplay::Table { headers, .. }, Some((0, col))) => {
            headers.get(col).map(String::as_str)
        }
        (FormBlockDisplay::Table { rows, .. }, Some((row, col))) => {
            rows.get(row - 1)?.get(col).map(String::as_str)
        }
        _ => None,
    }
}

mod block_view;
mod inline_toolbar;
mod table_view;

use block_view::FormBlockView;

#[cfg(test)]
mod tests {
    use super::*;
    use bekoedit_markdown::MarkdownIndex;

    fn projection_for(doc: &str) -> FormProjection {
        let index = MarkdownIndex::build(doc, 1);
        FormProjection::build(doc, &index)
    }

    #[test]
    fn an_identical_paragraph_replace_is_unchanged() {
        let projection = projection_for("A paragraph.\n");
        let block_id = projection.blocks[0].block_id;
        let edit = FormBlockEdit::ReplacePlainText {
            text: "A paragraph.".to_string(),
        };
        assert!(is_unchanged_text_edit(&projection, block_id, &edit));
    }

    #[test]
    fn a_changed_paragraph_replace_is_not_unchanged() {
        let projection = projection_for("A paragraph.\n");
        let block_id = projection.blocks[0].block_id;
        let edit = FormBlockEdit::ReplacePlainText {
            text: "A paragraph, changed.".to_string(),
        };
        assert!(!is_unchanged_text_edit(&projection, block_id, &edit));
    }

    #[test]
    fn an_identical_table_cell_replace_is_unchanged() {
        let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let block_id = projection
            .blocks
            .iter()
            .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
            .unwrap()
            .block_id;
        let edit = FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "1".to_string(),
        };
        assert!(is_unchanged_text_edit(&projection, block_id, &edit));
    }

    #[test]
    fn a_changed_table_cell_replace_is_not_unchanged() {
        let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
        let block_id = projection
            .blocks
            .iter()
            .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
            .unwrap()
            .block_id;
        let edit = FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "99".to_string(),
        };
        assert!(!is_unchanged_text_edit(&projection, block_id, &edit));
    }

    #[test]
    fn any_other_edit_kind_is_never_treated_as_unchanged() {
        let projection = projection_for("# Heading\n");
        let block_id = projection.blocks[0].block_id;
        let edit = FormBlockEdit::SetHeadingLevel { level: 2 };
        assert!(!is_unchanged_text_edit(&projection, block_id, &edit));
    }
}
