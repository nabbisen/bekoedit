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

use bekoedit_core::{AppState, SessionError, StoreError};
use bekoedit_markdown::{
    FormBlockDisplay, FormBlockEdit, FormEditCommand, FormEditError, FormProjection,
    fingerprint::BlockId,
};

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};
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
                    key: "{block_key(block.block_id)}",
                    block_id: block.block_id,
                    display: block.display.clone(),
                    revision,
                    lang,
                }
            }
        }
    }
}

/// `FormBlockView`'s own identity for Dioxus's keyed diffing -- the
/// ordinal plus the block's kind, never its `content_hash` (task 052
/// §2.1). A changed key makes Dioxus unmount the old component and
/// mount a new one: correct for a genuinely different block at that
/// position, but wrong for a text commit to the *same* block, which
/// changes `content_hash` on every keystroke it is eventually blurred
/// or flushed on -- that would destroy the focused `<textarea>` (or
/// table cell `<input>`) and lose focus right as the commit lands, the
/// regression task 051's own keyboard flush exposed.
fn block_key(block_id: BlockId) -> String {
    format!("{}-{:?}", block_id.ordinal, block_id.kind)
}

fn dispatch(
    mut state: Signal<AppState>,
    revision: u64,
    block_id: BlockId,
    mut toasts: Signal<Vec<Toast>>,
    lang: Lang,
    edit: FormBlockEdit,
) {
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
    // Task 056 §2.1: a refused edit used to vanish silently (`let _ =`),
    // against RFC-047's rule that a user command is run or reported,
    // never dropped -- this is exactly how task 055's `table_row_insert_and_delete`
    // read as "the Delete row click had no effect" instead of naming why.
    let result = state.write().edit_form(&cmd, now_ms());
    if let Some((trace_details, toast_key)) = edit_outcome(&result) {
        crate::bridge::trace("form.edit_refused", trace_details);
        push_toast(&mut toasts, ToastKind::Warning, tr(lang, toast_key));
    }
}

/// What `dispatch` does with `edit_form`'s own result (task 056 §2.1,
/// §4): the trace details to record and the toast key to show, or
/// `None` for `Ok(())` -- an edit that actually applied has nothing to
/// report. Pure, so every refusal kind is tested without a live
/// `Signal` at all.
fn edit_outcome(result: &Result<(), StoreError>) -> Option<(String, &'static str)> {
    let error = result.as_ref().err()?;
    Some((
        edit_refused_trace_details(error),
        edit_refused_toast_key(error),
    ))
}

/// What `bridge::trace` records for a refused Form edit: the error's own
/// kind, and for a revision mismatch, the expected and current
/// revision -- never the `reason` text `UnsupportedEditOperation`/
/// `InvalidEditPayload` carry, which is developer prose, not something
/// a trace line should depend on staying stable (task 056 §2.1, §3).
/// `edit_form`'s own signature is `Result<(), StoreError>` (`store.rs`),
/// not `FormEditError` directly -- every Form-shaped refusal arrives as
/// `StoreError::Session(SessionError::Form(_))`; every other
/// `StoreError` variant is a precondition `dispatch`'s own caller could
/// not have satisfied either (no open document, a pending conflict, …)
/// and gets its own, equally plain kind.
fn edit_refused_trace_details(error: &StoreError) -> String {
    match error {
        StoreError::Session(SessionError::Form(form_error)) => match form_error {
            FormEditError::DocumentRevisionMismatch { base, current } => {
                format!("kind=document_revision_mismatch base={base} current={current}")
            }
            FormEditError::BlockNotFound => "kind=block_not_found".to_string(),
            FormEditError::BlockFingerprintMismatch => {
                "kind=block_fingerprint_mismatch".to_string()
            }
            FormEditError::ItemNotFound { ordinal } => {
                format!("kind=item_not_found ordinal={ordinal}")
            }
            FormEditError::UnsupportedEditOperation { .. } => {
                "kind=unsupported_edit_operation".to_string()
            }
            FormEditError::InvalidEditPayload { .. } => "kind=invalid_edit_payload".to_string(),
        },
        StoreError::NoWorkspace => "kind=no_workspace".to_string(),
        StoreError::NoDocument => "kind=no_document".to_string(),
        StoreError::ConflictPending => "kind=conflict_pending".to_string(),
        StoreError::DocumentDirty => "kind=document_dirty".to_string(),
        StoreError::Untitled => "kind=untitled".to_string(),
        StoreError::Workspace(_) => "kind=workspace_error".to_string(),
        StoreError::Session(_) => "kind=session_error".to_string(),
        StoreError::FileOp(_) => "kind=file_op_error".to_string(),
        StoreError::SaveFailed(_) => "kind=save_failed".to_string(),
    }
}

/// The toast key for a refused Form edit, grouped by what the user can
/// do about it (task 056 §2.1): the document or the block moved under
/// them (try again); the thing they targeted is gone; or the change
/// itself is not one Form Mode supports here. Every non-Form
/// `StoreError` falls into the last bucket too: Form Mode has no
/// specific recovery story for "no document is open" beyond "this
/// didn't apply".
fn edit_refused_toast_key(error: &StoreError) -> &'static str {
    match error {
        StoreError::Session(SessionError::Form(form_error)) => match form_error {
            FormEditError::DocumentRevisionMismatch { .. }
            | FormEditError::BlockFingerprintMismatch
            | FormEditError::BlockNotFound => "form.edit_refused.stale",
            FormEditError::ItemNotFound { .. } => "form.edit_refused.item_gone",
            FormEditError::UnsupportedEditOperation { .. }
            | FormEditError::InvalidEditPayload { .. } => "form.edit_refused.unsupported",
        },
        _ => "form.edit_refused.unsupported",
    }
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
mod placement;
mod table_column_menu;
mod table_row_menu;
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

    /// Task 052 §2.1/§4: a text edit to a block must not change its own
    /// `FormBlockView` key -- otherwise Dioxus unmounts and remounts the
    /// field on every commit, losing focus. Mutation: put
    /// `block_id.fingerprint.content_hash` back into `block_key`, and
    /// this fails, since the two documents' paragraphs hash differently.
    #[test]
    fn a_text_edit_to_a_block_keeps_the_same_key() {
        let before = projection_for("A paragraph.\n");
        let after = projection_for("A paragraph, changed.\n");
        assert_eq!(
            block_key(before.blocks[0].block_id),
            block_key(after.blocks[0].block_id)
        );
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

    // --- Task 056: a refused Form edit produces a trace and a toast ---

    /// Every `FormEditError` kind produces a trace and a toast key --
    /// pure, no live `Signal` needed (task 056 §4's first required
    /// test). Mutation: go back to discarding `edit_form`'s own result
    /// outright in `dispatch`, and this whole path is unreachable; the
    /// far more direct mutation is removing the `if let Some(...)`
    /// around the trace/toast call, covered below.
    #[test]
    fn every_form_edit_error_kind_produces_a_trace_and_a_toast() {
        let cases: Vec<(StoreError, &str)> = vec![
            (
                StoreError::Session(SessionError::Form(
                    FormEditError::DocumentRevisionMismatch {
                        base: 1,
                        current: 2,
                    },
                )),
                "form.edit_refused.stale",
            ),
            (
                StoreError::Session(SessionError::Form(FormEditError::BlockNotFound)),
                "form.edit_refused.stale",
            ),
            (
                StoreError::Session(SessionError::Form(FormEditError::BlockFingerprintMismatch)),
                "form.edit_refused.stale",
            ),
            (
                StoreError::Session(SessionError::Form(FormEditError::ItemNotFound {
                    ordinal: 3,
                })),
                "form.edit_refused.item_gone",
            ),
            (
                StoreError::Session(SessionError::Form(
                    FormEditError::UnsupportedEditOperation { reason: "x".into() },
                )),
                "form.edit_refused.unsupported",
            ),
            (
                StoreError::Session(SessionError::Form(FormEditError::InvalidEditPayload {
                    reason: "x".into(),
                })),
                "form.edit_refused.unsupported",
            ),
            (StoreError::NoDocument, "form.edit_refused.unsupported"),
            (StoreError::ConflictPending, "form.edit_refused.unsupported"),
        ];
        for (error, expected_key) in cases {
            let (details, key) =
                edit_outcome(&Err(error)).expect("an Err must always produce an outcome");
            assert!(!details.is_empty(), "the trace details must name a kind");
            assert_eq!(key, expected_key);
            // Both languages resolve the key (the i18n coverage test
            // also checks this from every `tr(lang, "...")` call site,
            // but these keys are built, not written literally).
            assert!(!tr(crate::i18n::Lang::En, key).is_empty());
            assert!(!tr(crate::i18n::Lang::Ja, key).is_empty());
        }
    }

    /// The revision mismatch's own expected/current values reach the
    /// trace, not just its kind (task 056 §2.1's "for a mismatch, the
    /// expected and actual revision").
    #[test]
    fn a_revision_mismatch_traces_its_own_base_and_current() {
        let error = StoreError::Session(SessionError::Form(
            FormEditError::DocumentRevisionMismatch {
                base: 5,
                current: 9,
            },
        ));
        let details = edit_refused_trace_details(&error);
        assert!(details.contains("base=5"), "{details}");
        assert!(details.contains("current=9"), "{details}");
    }

    /// No `reason` text from `UnsupportedEditOperation`/
    /// `InvalidEditPayload` ever reaches the trace (task 056 §3: "never
    /// document text" -- these `reason` strings are the closest thing
    /// to document-adjacent free text this error type carries).
    #[test]
    fn the_reason_text_never_reaches_the_trace() {
        let error = StoreError::Session(SessionError::Form(
            FormEditError::UnsupportedEditOperation {
                reason: "FORBIDDEN-SECRET-REASON".into(),
            },
        ));
        let details = edit_refused_trace_details(&error);
        assert!(!details.contains("FORBIDDEN-SECRET-REASON"), "{details}");
    }

    /// An accepted edit (`Ok(())`) produces neither a trace nor a toast
    /// (task 056 §4's second required test).
    #[test]
    fn an_accepted_edit_produces_neither_trace_nor_toast() {
        assert!(edit_outcome(&Ok(())).is_none());
    }

    /// The identical-text guard returns before `edit_form` is ever
    /// called, so neither a trace nor a toast can fire for it (task 056
    /// §4's third required test; §2.1's two exceptions). Structural,
    /// since the guard itself needs a live `AppState` with a session to
    /// exercise end to end -- what matters here is that the early
    /// `return` in `dispatch`'s own source precedes the `edit_form`
    /// call it would otherwise reach. Mutation: move the guard's
    /// `return;` after the `edit_form` call, and this fails.
    #[test]
    fn the_unchanged_text_guard_returns_before_edit_form_is_ever_called() {
        let source = include_str!("form_mode.rs");
        let guard_at = source
            .find("is_unchanged_text_edit(&projection, block_id, &edit)")
            .expect("the guard condition");
        let return_at = source[guard_at..]
            .find("return;")
            .map(|i| guard_at + i)
            .expect("the guard's own early return");
        let edit_form_at = source
            .find("state.write().edit_form(&cmd, now_ms())")
            .expect("the edit_form call");
        assert!(
            return_at < edit_form_at,
            "the identical-text guard must return before edit_form is ever called"
        );
    }
}
