//! A single, globally-ordered path for every command submission (review,
//! 2026-10-02 §2.3). `submit_source_command`/`submit_source_interaction`/
//! `submit_source_shortcut_interaction` used to `spawn` their own task per
//! call, each awaiting its own round trip (the pending-field commit's
//! `eval_body`, or the focus-guard's own arm exchange) before reaching
//! `submit_with_focus`. Two submissions fired in quick succession then
//! raced: whichever task's own round trip resolved first reached
//! `submit_with_focus` first, regardless of which the user actually
//! issued first.
//!
//! Fixed by making submission itself synchronous and ordered: every path
//! now calls [`enqueue`] -- a plain, non-async send into an unbounded
//! channel, so two calls in the same tick land in the channel in the
//! exact order they were made -- and one coroutine, mounted once
//! ([`SourceCommandQueue`]), drains the channel and `.await`s each
//! [`Submission`] fully before taking the next.
//!
//! **Only the async remainder is queued** (the pending-field commit;
//! for one that claims editor focus, the arm sequence too). The
//! re-review (§2) found that queueing the *decision* of whether a
//! submission claims focus at all -- not just its async remainder --
//! broke two things that depend on that decision being made at the same
//! instant the user's input arrived: `claims_focus`'s own prediction
//! (`handoff.rs`), and a later direct command's synchronous
//! `cancel_focus_interactions` correctly cancelling an earlier click's
//! claim. [`Submission`] therefore always carries an *already-decided*
//! outcome -- [`Submission::WithFocusClaim`] carries the token
//! `submit_interaction` already allocated, synchronously, before
//! enqueueing.

use std::cell::RefCell;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;
use futures_util::StreamExt;

use crate::components::toast::Toast;
use crate::i18n::Lang;

use super::focus::SourceInteractionOrigin;
use super::{AppSignals, SourceCommand, SourceSyncState};

/// One submission, with its focus-claim decision already made,
/// synchronously, by `submit_interaction` itself (re-review §2) --
/// `sync`/`state`/`mode`/`toasts`/`lang` are not part of this type
/// because they are the same signals throughout the app; the consumer
/// captures them once, from its own component context.
pub(super) enum Submission {
    /// No focus claim: a direct command, or an interaction that decided,
    /// at submission time, not to claim one after all.
    WithoutFocusClaim { command: SourceCommand },
    /// `token`/`fingerprint` were already allocated for `target`,
    /// synchronously, before this was enqueued. Only the arm sequence
    /// (needing a real round trip) and the submit itself are queued.
    WithFocusClaim {
        command: SourceCommand,
        origin: SourceInteractionOrigin,
        token: u64,
        fingerprint: String,
        finalize_launch_ui: Box<dyn FnOnce()>,
    },
}

// `thread_local!`, not a `static` `OnceLock`: `Submission` carries a boxed
// `FnOnce()` (`finalize_launch_ui`), which is under no obligation to be
// `Send`, and a `OnceLock`'s contents must be `Sync` even though this
// desktop app's UI, and every task `spawn`ed from it, all run on the one
// thread Dioxus itself uses -- a `thread_local!` needs neither bound.
thread_local! {
    static QUEUE: RefCell<Option<UnboundedSender<Submission>>> = const { RefCell::new(None) };
}

/// Enqueues `submission`. A no-op (never panics) if
/// [`SourceCommandQueue`] has not mounted yet -- it mounts once, from
/// `App`'s own body, before any other component that could call this.
pub(super) fn enqueue(submission: Submission) {
    QUEUE.with_borrow(|tx| {
        if let Some(tx) = tx {
            let _ = tx.unbounded_send(submission);
        }
    });
}

/// Mounted once, from `App`'s own body (`app.rs`): the one place that
/// already owns `AppState`/`SourceSyncState`/`EditorMode`/the toast
/// list/`Lang` as top-level signals, which this captures once and reuses
/// for every submission it will ever process, strictly in the order
/// [`enqueue`] received them.
#[component]
pub fn SourceCommandQueue() -> Element {
    let signals = AppSignals {
        sync: use_context::<Signal<SourceSyncState>>(),
        state: use_context::<Signal<AppState>>(),
        mode: use_context::<Signal<EditorMode>>(),
        toasts: use_context::<Signal<Vec<Toast>>>(),
        lang: use_context::<Signal<Lang>>(),
    };

    let coroutine = use_coroutine(move |mut rx: UnboundedReceiver<Submission>| async move {
        while let Some(submission) = rx.next().await {
            match submission {
                Submission::WithoutFocusClaim { command } => {
                    super::process_direct_submission(signals, command).await;
                }
                Submission::WithFocusClaim {
                    command,
                    origin,
                    token,
                    fingerprint,
                    finalize_launch_ui,
                } => {
                    super::focus::run_interaction(
                        signals,
                        command,
                        origin,
                        token,
                        fingerprint,
                        finalize_launch_ui,
                    )
                    .await;
                }
            }
        }
    });
    use_hook(move || {
        QUEUE.with_borrow_mut(|tx| *tx = Some(coroutine.tx()));
    });

    rsx! {}
}

#[cfg(test)]
mod tests;
