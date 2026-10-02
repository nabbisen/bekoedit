//! Commits a Form Mode field's pending (typed, not yet blurred) text before
//! any command that saves or leaves the document's current state runs
//! (task 048 D1). A Form field commits only on `onchange`, which fires on
//! blur -- a keyboard-triggered command (Ctrl+S, a shortcut mode switch)
//! never touches DOM focus, so without this the field's pending edit is
//! silently discarded.
//!
//! Review (2026-10-02 §2.2): mid-composition, this reports [`CommitOutcome::Composing`]
//! and commits nothing, but it is the *caller*'s job to refuse the command
//! itself in that case -- running it anyway (saving without the pending
//! text, including text typed before the composition even started) was
//! the bug this task exists to fix, not an acceptable fallback.

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockDisplay, FormBlockEdit, FormEditCommand, FormProjection};
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;
use serde::Deserialize;

use crate::state::now_ms;

/// Finds the focused Form Mode field, if any, and reads its live value and
/// whether an IME composition is in progress, in one bounded round trip
/// (`bridge::eval_body`). `null` (decodes to `None`) whenever nothing
/// inside `.form-mode` has focus.
const PENDING_FIELD_JS: &str = "\
    const el = document.activeElement; \
    if (!el || typeof el.value !== 'string' || !el.closest || !el.closest('.form-mode')) return null; \
    const id = el.id || ''; \
    if (!id.startsWith('fb-')) return null; \
    return { id, value: el.value, composing: window.__bk_form_composing === true };";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PendingField {
    id: String,
    value: String,
    composing: bool,
}

/// A parsed `fb-{ordinal}-{content_hash}` (`block_view.rs`), with `cell`
/// set if `id` instead named one table cell:
/// `fb-{ordinal}-{content_hash}-{row}-{col}` (`table_view.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedFieldId {
    ordinal: u32,
    content_hash: u64,
    cell: Option<(usize, usize)>,
}

fn parse_field_id(id: &str) -> Option<ParsedFieldId> {
    let parts: Vec<&str> = id.split('-').collect();
    if parts.first() != Some(&"fb") {
        return None;
    }
    let (ordinal, content_hash) = (parts.get(1)?.parse().ok()?, parts.get(2)?.parse().ok()?);
    let cell = match parts.len() {
        3 => None,
        5 => Some((parts[3].parse().ok()?, parts[4].parse().ok()?)),
        _ => return None,
    };
    Some(ParsedFieldId {
        ordinal,
        content_hash,
        cell,
    })
}

/// The current text at `cell` (or the whole field, if `cell` is `None`),
/// read from the live projection -- so a pending value that already
/// matches what is committed never dispatches a no-op edit. `None` if
/// `display`/`cell` do not agree on what kind of field this is (a block
/// whose kind changed under a stale id, or a cell address the table no
/// longer has).
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

/// Why [`resolve_pending_commit`] found nothing to dispatch -- task 049
/// §2.2: distinguished so `commit_pending_form_field` can trace *which*
/// of these happened, not just that nothing committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoCommitReason {
    /// `parse_field_id` rejected the pending field's own `id`.
    IdDidNotParse,
    /// The id parsed, but no block in the live projection has that
    /// ordinal and content hash.
    NoMatchingBlock,
    /// The block matched, but `current_text` could not read a value from
    /// it at the given cell address (a block whose kind changed under a
    /// stale id, or a cell address the table no longer has).
    KindsDidNotMatch,
    /// The field's live value already matches what is committed.
    ValueUnchanged,
}

impl NoCommitReason {
    /// The exact wording task 049 §2.2 names for each reason, so the
    /// trace and this type's own documentation cannot drift apart.
    pub(crate) const fn trace_label(self) -> &'static str {
        match self {
            Self::IdDidNotParse => "the id did not parse",
            Self::NoMatchingBlock => "no block matched",
            Self::KindsDidNotMatch => "the kinds did not match",
            Self::ValueUnchanged => "the value was unchanged",
        }
    }
}

/// Builds the commit command for `pending` against `projection` -- pure, so
/// it is tested headlessly without a live WebView. `Err` when nothing
/// should be dispatched, naming why (task 049 §2.2).
fn resolve_pending_commit(
    pending: &PendingField,
    projection: &FormProjection,
) -> Result<FormEditCommand, NoCommitReason> {
    let ParsedFieldId {
        ordinal,
        content_hash,
        cell,
    } = parse_field_id(&pending.id).ok_or(NoCommitReason::IdDidNotParse)?;
    let block = projection
        .blocks
        .iter()
        .find(|b| {
            b.block_id.ordinal == ordinal && b.block_id.fingerprint.content_hash == content_hash
        })
        .ok_or(NoCommitReason::NoMatchingBlock)?;
    let stored = current_text(&block.display, cell).ok_or(NoCommitReason::KindsDidNotMatch)?;
    if stored == pending.value {
        return Err(NoCommitReason::ValueUnchanged);
    }
    let edit = match cell {
        None => FormBlockEdit::ReplacePlainText {
            text: pending.value.clone(),
        },
        Some((row, col)) => FormBlockEdit::ReplaceTableCell {
            row,
            col,
            text: pending.value.clone(),
        },
    };
    Ok(FormEditCommand {
        base_revision: projection.document_revision,
        block_id: block.block_id,
        client_block_fingerprint: Some(block.block_id.fingerprint),
        edit,
    })
}

/// What happened when a command tried to commit a Form Mode field's
/// pending text before running (task 048 D1, extended by the review's
/// §2.2). The caller decides what to do with each variant; this function
/// only observes and, when it can, commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitOutcome {
    /// A pending edit was found and committed.
    Committed,
    /// Nothing needed committing: not in Form Mode, no field had focus,
    /// or its value already matched what was committed.
    NothingPending,
    /// A composition is in progress. Nothing was committed -- forcing one
    /// now would write an intermediate, not-yet-confirmed string, the
    /// same reason Text Mode never does this either
    /// (`source_sync/lifecycle/transitions.rs`'s `CompositionActive`
    /// handling). **The caller must refuse the command itself**, not run
    /// it without the pending text.
    Composing,
}

/// Commits whatever Form Mode field currently has focus and holds
/// uncommitted text, before the caller goes on to run a command that saves
/// or otherwise acts on the document's current state (task 048 D1). A
/// no-op outside Form Mode, if nothing has focus inside it, or if the
/// value already matches what is committed.
pub(crate) async fn commit_pending_form_field(
    mut state: Signal<AppState>,
    mode: Signal<EditorMode>,
) -> CommitOutcome {
    if *mode.read() != EditorMode::Form {
        crate::bridge::trace("source.form_commit.skip", "mode is not Form");
        return CommitOutcome::NothingPending;
    }
    let read = crate::bridge::eval_body::<Option<PendingField>>(PENDING_FIELD_JS).await;
    let pending = match read {
        Ok(Some(pending)) => {
            crate::bridge::trace(
                "source.form_commit.read",
                format!(
                    "id={} value_len={} composing={}",
                    pending.id,
                    pending.value.chars().count(),
                    pending.composing
                ),
            );
            pending
        }
        Ok(None) => {
            crate::bridge::trace(
                "source.form_commit.read",
                "null (nothing in .form-mode has focus)",
            );
            return CommitOutcome::NothingPending;
        }
        Err(error) => {
            crate::bridge::trace("source.form_commit.read.error", error);
            return CommitOutcome::NothingPending;
        }
    };
    if pending.composing {
        crate::bridge::trace("source.form_commit.composing", &pending.id);
        return CommitOutcome::Composing;
    }
    let Some(projection) = state.read().session.as_ref().map(|s| s.form_projection()) else {
        crate::bridge::trace("source.form_commit.skip", "no open session");
        return CommitOutcome::NothingPending;
    };
    match resolve_pending_commit(&pending, &projection) {
        Ok(cmd) => match state.write().edit_form(&cmd, now_ms()) {
            Ok(()) => {
                crate::bridge::trace("source.form_commit.committed", &pending.id);
                CommitOutcome::Committed
            }
            Err(error) => {
                crate::bridge::trace("source.form_commit.edit_form.error", error);
                CommitOutcome::NothingPending
            }
        },
        Err(reason) => {
            crate::bridge::trace("source.form_commit.no_commit", reason.trace_label());
            CommitOutcome::NothingPending
        }
    }
}

#[cfg(test)]
mod tests;
