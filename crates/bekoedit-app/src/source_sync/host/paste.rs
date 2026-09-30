//! RFC-046 §3.2: converts one pasted `text/html` flavour and replies.
//!
//! `SourceEditorEvent::PasteRequested` arrives over the same persistent relay
//! every other page-originated event does (`host.rs`'s coroutine); this module
//! is what `host.rs` calls for it, kept separate so `host.rs` stays under the
//! ELOC limit. It never touches `SourceSyncState`: a paste is not a lifecycle
//! transition (the RFC-041 mount/snapshot/resume state machine), and its
//! result is applied by the page as an ordinary CodeMirror edit, which flows
//! back through the *existing* `Change` event like any keystroke. Rust's part
//! is stateless: one request in, one reply out (or, for a paste already
//! decided locally, one notice and no reply).

use bekoedit_paste::{FallbackReason, LineEnding, Outcome, convert_with_marker};
use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{EditorIdentity, PasteFallbackReason, PasteOutcome, SourceEditorRequest},
};
use dioxus::prelude::*;

use crate::{
    bridge,
    components::toast::{Toast, ToastKind, push_toast},
    i18n::{Lang, tr},
};

use super::dispatch_request;

/// `SourceEditorEvent::PasteRequested` arrived. `html` is `None` when the page
/// already decided `TooLarge` itself and inserted the plain flavour: there is
/// nothing to convert and no reply to send, only the notice.
pub(super) fn handle_paste_requested(
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
        if let Some(key) = notice_key {
            notify(toasts, lang, key);
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

/// `SourceEditorEvent::PasteDiscarded` arrived (RFC-046 §3.3): the page could
/// not apply a `PasteResult`, so it discarded it. Raises the one Warning
/// notice that discard requires; there is nothing else to do.
pub(super) fn handle_paste_discarded(token: u64, mut toasts: Signal<Vec<Toast>>, lang: Lang) {
    bridge::trace("source.paste.discarded", format!("token={token}"));
    push_toast(&mut toasts, ToastKind::Warning, tr(lang, "paste.discarded"));
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
