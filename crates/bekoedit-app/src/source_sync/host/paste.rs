//! RFC-046 §3.2: converts one pasted `text/html` flavour and replies.
//!
//! `SourceEditorEvent::PasteRequested` arrives over the same persistent relay
//! every other page-originated event does (`host.rs`'s coroutine); this module
//! is what `host.rs` calls for it, kept separate so `host.rs` stays under the
//! ELOC limit. It never touches `SourceSyncState`: a paste is not a lifecycle
//! transition (the RFC-041 mount/snapshot/resume state machine), and its
//! result is applied by the page as an ordinary CodeMirror edit, which flows
//! back through the *existing* `Change` event like any keystroke.
//!
//! **Exactly one notice per paste (§3.4), raised after the page has decided**
//! what happened to the reply, not when Rust sends it: a reply's notice is
//! held in [`PENDING_NOTICES`] until the page says `PasteApplied` (raise it)
//! or `PasteDiscarded` (drop it, and raise `paste.discarded` instead). Two
//! notices for one paste would otherwise be possible -- a fallback reason
//! raised on send, and then a contradictory discard notice a moment later,
//! for content that was never actually inserted.
//!
//! **Known residual, recorded, not fixed:** if the editor remounts while a
//! conversion is in flight, neither `PasteApplied` nor `PasteDiscarded` ever
//! arrives for that token under the identity it was set under (the page's own
//! token numbering restarts on a remount, so a later paste on the new editor
//! could otherwise reuse the same token number). Keyed by `(EditorIdentity,
//! token)`, not token alone (review, 2026-09-30), that stale entry can never
//! be taken by a different editor's paste; it just ages out of the bounded
//! store, unraised. This is rare -- conversion takes on the order of tens of
//! milliseconds -- and a silent notice loss on an already-unusual remount is
//! preferable to inventing a timeout for it.

use std::sync::Mutex;

use bekoedit_paste::{FallbackReason, LineEnding, Outcome, convert_with_marker};
use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, PasteFallbackReason, PasteOutcome, SourceEditorEvent, SourceEditorRequest,
    },
};
use dioxus::prelude::*;

use crate::{
    bridge,
    components::toast::{Toast, ToastKind, push_toast},
    i18n::{Lang, tr},
};

use super::dispatch_request;

/// A paste's notice key, held between the reply being sent and the page
/// saying what happened to it. Keyed by `(EditorIdentity, token)`, not token
/// alone (review, 2026-09-30): `paste.js` numbers tokens from 1 per editor
/// script instance, so a remount restarts that numbering, and a token-only
/// key could let a stale entry be taken by an unrelated later paste on the
/// new editor, showing that paste's user someone else's notice. Bounded (the
/// residual above): the oldest entry is evicted first, so a run of abandoned
/// entries cannot grow this without limit. A plain type, not the `static`
/// below directly, so its own tests exercise an isolated instance rather
/// than the one every handler shares -- eviction order is otherwise not
/// deterministic to test against a process-global store under a parallel
/// test run.
#[derive(Debug, Default)]
struct PendingNotices {
    entries: Vec<(EditorIdentity, u64, &'static str)>,
}

impl PendingNotices {
    fn set(&mut self, identity: EditorIdentity, token: u64, key: &'static str) {
        if self.entries.len() >= MAX_PENDING_NOTICES {
            self.entries.remove(0);
        }
        self.entries.push((identity, token, key));
    }

    /// Removes and returns `(identity, token)`'s pending notice, if it still
    /// has one. Called exactly once per paste, by whichever of
    /// `PasteApplied`/`PasteDiscarded` arrives first for it.
    fn take(&mut self, identity: EditorIdentity, token: u64) -> Option<&'static str> {
        let index = self
            .entries
            .iter()
            .position(|(i, t, _)| *i == identity && *t == token)?;
        Some(self.entries.remove(index).2)
    }
}

const MAX_PENDING_NOTICES: usize = 32;
static PENDING_NOTICES: Mutex<PendingNotices> = Mutex::new(PendingNotices {
    entries: Vec::new(),
});

fn set_pending_notice(identity: EditorIdentity, token: u64, key: &'static str) {
    PENDING_NOTICES
        .lock()
        .expect("pending-notices lock")
        .set(identity, token, key);
}

fn take_pending_notice(identity: EditorIdentity, token: u64) -> Option<&'static str> {
    PENDING_NOTICES
        .lock()
        .expect("pending-notices lock")
        .take(identity, token)
}

/// None of RFC-046's paste events are lifecycle transitions (module doc
/// comment), so this fully handles the three of them and hands back `None`;
/// anything else it hands back unchanged, for `host.rs`'s relay loop to pass
/// to `handle_event` as before. The single entry point from `host.rs`, so the
/// three `handle_paste_*` functions below need no visibility past this module.
pub(super) fn intercept(
    event: SourceEditorEvent,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
    relay_generation: u64,
) -> Option<SourceEditorEvent> {
    match event {
        SourceEditorEvent::PasteRequested {
            identity,
            token,
            html,
            plain_length,
            ..
        } => {
            handle_paste_requested(
                identity,
                token,
                html,
                plain_length,
                toasts,
                lang,
                relay_generation,
            );
            None
        }
        SourceEditorEvent::PasteDiscarded {
            identity, token, ..
        } => {
            handle_paste_discarded(identity, token, toasts, lang);
            None
        }
        SourceEditorEvent::PasteApplied {
            identity, token, ..
        } => {
            handle_paste_applied(identity, token, toasts, lang);
            None
        }
        other => Some(other),
    }
}

/// `SourceEditorEvent::PasteRequested` arrived. `html` is `None` when the page
/// already decided `TooLarge` itself and inserted the plain flavour: there is
/// nothing to convert and no reply to send, only the notice.
fn handle_paste_requested(
    identity: EditorIdentity,
    token: u64,
    html: Option<String>,
    plain_length: u64,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
    relay_generation: u64,
) {
    bridge::trace(
        "source.paste.requested",
        format!(
            "token={token} plain_length={plain_length} decided_locally={}",
            html.is_none()
        ),
    );
    let Some(html) = html else {
        notify(toasts, lang, "paste.too_large");
        return;
    };
    let marker = tr(lang, "paste.image_omitted").to_string();
    spawn(async move {
        // Off the UI thread (RFC-046 §3.2): `bekoedit_paste::convert` blocks
        // its caller for up to its own 2 s budget while it waits on the
        // worker thread it spawns internally; `spawn_blocking` keeps that
        // wait off Dioxus's async executor.
        let outcome = tokio::task::spawn_blocking(move || convert(&html, &marker))
            .await
            .unwrap_or(Outcome::Fallback(FallbackReason::Failed));
        let (wire_outcome, notice_key) = classify(outcome);
        // Held, not raised: the page has not yet decided whether this reply
        // is applied or discarded (module doc comment).
        if let Some(key) = notice_key {
            set_pending_notice(identity, token, key);
        }
        bridge::trace(
            "source.paste.replied",
            format!("token={token} outcome={wire_outcome:?}"),
        );
        let request = SourceEditorRequest::PasteResult {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            identity,
            token,
            outcome: wire_outcome,
        };
        dispatch_request(&request, None, relay_generation);
    });
}

/// The wire outcome, and the i18n key of the one notice it raises, if any
/// (RFC-046 §3.4). Pure, so every case is unit tested without a real convert.
pub(super) fn classify(outcome: Outcome) -> (PasteOutcome, Option<&'static str>) {
    match outcome {
        Outcome::Converted {
            markdown,
            html_had_table,
        } => {
            let no_gfm_form = html_had_table && !bekoedit_markdown::has_gfm_table(&markdown);
            let notice = no_gfm_form.then_some("paste.table_no_form");
            (
                PasteOutcome::Converted {
                    markdown,
                    table_no_gfm_form: no_gfm_form,
                },
                notice,
            )
        }
        Outcome::Fallback(reason) => {
            let (wire, key) = match reason {
                FallbackReason::TooLarge => (PasteFallbackReason::TooLarge, "paste.too_large"),
                FallbackReason::Failed => (PasteFallbackReason::Failed, "paste.failed"),
                FallbackReason::TimedOut => (PasteFallbackReason::TimedOut, "paste.timed_out"),
            };
            (PasteOutcome::Fallback { reason: wire }, Some(key))
        }
        Outcome::Empty => (PasteOutcome::Empty, None),
    }
}

fn notify(mut toasts: Signal<Vec<Toast>>, lang: Lang, key: &'static str) {
    push_toast(&mut toasts, ToastKind::Info, tr(lang, key));
}

/// What arriving at a decision for one token (`PasteApplied` or
/// `PasteDiscarded`) should raise, if anything.
#[derive(Debug, Clone, PartialEq)]
enum PendingResolution {
    Notify(&'static str, ToastKind),
    Nothing,
}

/// The whole "exactly one notice, and which one" decision (RFC-046 §3.4) for
/// each side, as pure functions over [`PENDING_NOTICES`]: no `Signal`
/// involved, so every case -- including that a discarded reply's own notice
/// never surfaces -- is unit tested directly.
fn resolve_applied(identity: EditorIdentity, token: u64) -> PendingResolution {
    match take_pending_notice(identity, token) {
        Some(key) => PendingResolution::Notify(key, ToastKind::Info),
        None => PendingResolution::Nothing,
    }
}

fn resolve_discarded(identity: EditorIdentity, token: u64) -> PendingResolution {
    // Dropped, not surfaced: it would be false -- nothing was pasted.
    take_pending_notice(identity, token);
    PendingResolution::Notify("paste.discarded", ToastKind::Warning)
}

/// `SourceEditorEvent::PasteApplied` arrived (RFC-046 §3.4): the page
/// inserted the reply's result. Raises that reply's held notice, if it had
/// one; a successful conversion or `Empty` has none.
fn handle_paste_applied(
    identity: EditorIdentity,
    token: u64,
    mut toasts: Signal<Vec<Toast>>,
    lang: Lang,
) {
    bridge::trace("source.paste.applied", format!("token={token}"));
    if let PendingResolution::Notify(key, kind) = resolve_applied(identity, token) {
        push_toast(&mut toasts, kind, tr(lang, key));
    }
}

/// `SourceEditorEvent::PasteDiscarded` arrived (RFC-046 §3.3): the page could
/// not apply a `PasteResult`, so it discarded it.
fn handle_paste_discarded(
    identity: EditorIdentity,
    token: u64,
    mut toasts: Signal<Vec<Toast>>,
    lang: Lang,
) {
    bridge::trace("source.paste.discarded", format!("token={token}"));
    if let PendingResolution::Notify(key, kind) = resolve_discarded(identity, token) {
        push_toast(&mut toasts, kind, tr(lang, key));
    }
}

/// RFC-046 §3.3: `LineEnding::Lf`, always -- the editor speaks editor form,
/// and task 027's reconciliation gives every inserted line break the file's
/// own ending, so a `Mixed` file needs no special mapping here. A pure
/// wrapper so that choice is itself unit tested, without a real WebView.
fn convert(html: &str, marker: &str) -> Outcome {
    convert_with_marker(html, "", LineEnding::Lf, marker)
}

#[cfg(test)]
mod tests;
