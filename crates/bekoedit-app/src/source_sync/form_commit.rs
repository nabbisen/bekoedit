//! Commits a Form Mode field's pending (typed, not yet blurred) text before
//! any command that saves or leaves the document's current state runs
//! (task 048 D1). A Form field commits only on `onchange`, which fires on
//! blur -- a keyboard-triggered command (Ctrl+S, a shortcut mode switch)
//! never touches DOM focus, so without this the field's pending edit is
//! silently discarded.

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

/// Builds the commit command for `pending` against `projection` -- pure, so
/// it is tested headlessly without a live WebView. `None` when nothing
/// should be dispatched: the id does not resolve to a block the live
/// projection still has, or the field's value already matches what is
/// committed.
fn resolve_pending_commit(
    pending: &PendingField,
    projection: &FormProjection,
) -> Option<FormEditCommand> {
    let ParsedFieldId {
        ordinal,
        content_hash,
        cell,
    } = parse_field_id(&pending.id)?;
    let block = projection.blocks.iter().find(|b| {
        b.block_id.ordinal == ordinal && b.block_id.fingerprint.content_hash == content_hash
    })?;
    if current_text(&block.display, cell)? == pending.value {
        return None;
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
    Some(FormEditCommand {
        base_revision: projection.document_revision,
        block_id: block.block_id,
        client_block_fingerprint: Some(block.block_id.fingerprint),
        edit,
    })
}

/// Commits whatever Form Mode field currently has focus and holds
/// uncommitted text, before the caller goes on to run a command that saves
/// or otherwise acts on the document's current state (task 048 D1). A
/// no-op outside Form Mode, if nothing has focus inside it, or if the
/// value already matches what is committed.
///
/// **Never commits mid-IME-composition.** Japanese input is a first-class
/// case here: forcing a commit while a composition is still open would
/// write an intermediate, not-yet-confirmed string. The existing Text Mode
/// rule is the precedent (`source_sync/lifecycle/transitions.rs`'s
/// `CompositionActive` handling: the command is refused, not queued to
/// retry) -- the same choice here: a command that arrives mid-composition
/// commits nothing and proceeds with the document exactly as it already
/// was, rather than waiting on an event that may never come.
pub(crate) async fn commit_pending_form_field(
    mut state: Signal<AppState>,
    mode: Signal<EditorMode>,
) {
    if *mode.read() != EditorMode::Form {
        return;
    }
    let Ok(Some(pending)) =
        crate::bridge::eval_body::<Option<PendingField>>(PENDING_FIELD_JS).await
    else {
        return;
    };
    if pending.composing {
        return;
    }
    let Some(projection) = state.read().session.as_ref().map(|s| s.form_projection()) else {
        return;
    };
    if let Some(cmd) = resolve_pending_commit(&pending, &projection) {
        let _ = state.write().edit_form(&cmd, now_ms());
    }
}

#[cfg(test)]
mod tests;
