//! Application-root source-editor lifecycle and synchronization controller.

use std::path::PathBuf;

use bekoedit_core::{AppState, StoreError};
use bekoedit_fs::HistoryEntry;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::Lang;
use crate::state::now_ms;

mod commands;
mod controller;
mod discard_report;
mod focus;
mod form_commit;
mod handoff;
pub mod host;
pub mod lifecycle;
mod queue;

pub use bekoedit_ui_contract::source_editor::SourceEditorId;
pub use controller::{
    DiscardReason, EditorMountHandle, MountOutcome, QueueDiscard, SourceSyncState, SubmitOutcome,
};
pub use focus::{
    SourceInteractionOrigin, cancel_pending_source_focus, cancel_source_focus,
    submit_source_interaction, submit_source_shortcut_interaction,
};
pub use handoff::submit_handoff_activation;
pub use lifecycle::MountIntent;
pub use queue::SourceCommandQueue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    SwitchMode(EditorMode),
    OpenSettings,
    SaveNow,
    SaveAs(PathBuf),
    OpenDocument(PathBuf),
    NewUntitled,
    OpenWorkspace(PathBuf),
    CloseWorkspace,
    RestoreHistory(HistoryEntry),
    MoveSectionUp(usize),
    MoveSectionDown(usize),
}

/// The five signals every submission's processing needs, bundled so
/// passing them from one stage to the next (review, 2026-10-02 §2.3: the
/// single consumer now threads them through `process_direct_submission`/
/// `run_interaction`/`submit_source_command_preserving_focus`) does not
/// by itself trip clippy's argument-count limit. `SourceCommandQueue`
/// captures one of these, once, from its own component context, and
/// reuses it for every submission it will ever process.
#[derive(Clone, Copy)]
pub(super) struct AppSignals {
    pub(super) sync: Signal<SourceSyncState>,
    pub(super) state: Signal<AppState>,
    pub(super) mode: Signal<EditorMode>,
    pub(super) toasts: Signal<Vec<Toast>>,
    pub(super) lang: Signal<Lang>,
}

#[derive(Debug)]
pub enum SourceSyncError {
    NoDocument,
    ConflictPending,
    Busy,
    RevisionDrift,
    CompositionActive,
    EditorUnavailable,
    IdentityMismatch,
    UnsupportedVersion,
    Timeout,
    Store(StoreError),
    Transition(lifecycle::TransitionError),
}

/// Enqueues synchronously (review, 2026-10-02 §2.3): every submission
/// reaches [`queue::enqueue`] before this function returns, so two calls
/// in the same tick land in submission order, regardless of how long
/// either one's own later processing (the pending-field commit's round
/// trip) takes. [`SourceCommandQueue`] does the actual work, one
/// submission at a time, in that same order.
pub fn submit_source_command(
    mut sync: Signal<SourceSyncState>,
    _state: Signal<AppState>,
    _mode: Signal<EditorMode>,
    _toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        focus::cancel_focus_guards_through(token);
    }
    queue::enqueue(queue::Submission::Direct { command });
}

/// What [`queue::SourceCommandQueue`] does for a [`queue::Submission::Direct`],
/// in the order its `enqueue` call landed. The direct path's old body,
/// unchanged, just no longer `spawn`ed per call.
pub(super) async fn process_direct_submission(signals: AppSignals, command: SourceCommand) {
    submit_source_command_preserving_focus(signals, command, None).await;
}

/// Task 048 §2.2: a command that arrives mid-IME-composition is refused
/// outright, exactly as Text Mode's own `CompositionActive` handling
/// refuses one -- never run without the field's pending text, and never
/// queued to retry after the composition ends.
fn refuse_while_composing(toasts: &mut Signal<Vec<Toast>>, command: &SourceCommand, lang: Lang) {
    let discard = QueueDiscard {
        command: command.clone(),
        reason: DiscardReason::Composing,
        focus_token: None,
    };
    for message in discard_report::discard_messages(std::slice::from_ref(&discard), lang) {
        push_toast(toasts, ToastKind::Warning, message);
    }
}

/// Task 048 §2.2's decision, pulled out pure: the one `CommitOutcome` that
/// must stop the command from running at all, never run it anyway. Tested
/// directly, since the real decision site needs a live `eval_body` round
/// trip to reach.
fn should_refuse(outcome: form_commit::CommitOutcome) -> bool {
    outcome == form_commit::CommitOutcome::Composing
}

pub(super) async fn submit_source_command_preserving_focus(
    signals: AppSignals,
    command: SourceCommand,
    focus_token: Option<u64>,
) -> SubmitOutcome {
    let AppSignals {
        mut sync,
        state,
        mode,
        mut toasts,
        lang,
    } = signals;
    // Task 048 D1: ordered before the command runs, not timed -- the
    // `.await` below is this function's only one, so nothing else can run
    // between the commit landing and `submit_with_focus` being called.
    let commit_outcome = form_commit::commit_pending_form_field(state, mode).await;
    if should_refuse(commit_outcome) {
        refuse_while_composing(&mut toasts, &command, *lang.read());
        return SubmitOutcome::NoOp;
    }
    let document_id = state
        .read()
        .session
        .as_ref()
        .map(|session| session.document_id);
    let outcome = sync
        .write()
        .submit_with_focus(command, document_id, now_ms(), focus_token);
    match outcome {
        SubmitOutcome::NoOp
        | SubmitOutcome::ExecuteQueued
        | SubmitOutcome::SnapshotRequested(_)
        | SubmitOutcome::WaitingForReady
        | SubmitOutcome::Queued => {}
        // The refused command is in `drain_discards`, which the host reports.
        SubmitOutcome::QueueFull => {
            crate::bridge::trace("source.controller.queue.full", "");
        }
        SubmitOutcome::Unavailable => push_toast(
            &mut toasts,
            ToastKind::Error,
            SourceSyncError::EditorUnavailable.to_string(),
        ),
    }
    outcome
}

pub fn mount_source_editor(
    mut sync: Signal<SourceSyncState>,
    editor_id: SourceEditorId,
    document_id: u64,
    revision: u64,
) -> MountOutcome {
    sync.write().mount(
        MountIntent {
            editor_id,
            document_id,
            revision,
        },
        now_ms(),
    )
}

pub fn unmount_source_editor(mut sync: Signal<SourceSyncState>, handle: EditorMountHandle) {
    sync.write().unmount(handle, now_ms());
}

impl std::fmt::Display for SourceSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDocument => write!(f, "no document is open"),
            Self::ConflictPending => write!(f, "resolve the file conflict first"),
            Self::Busy => write!(f, "another source operation is still syncing"),
            Self::RevisionDrift => {
                write!(f, "the document changed before source sync finished")
            }
            Self::CompositionActive => write!(f, "finish composing text before this action"),
            Self::EditorUnavailable => write!(f, "the source editor is unavailable; retry"),
            Self::IdentityMismatch => write!(f, "the source editor identity did not match"),
            Self::UnsupportedVersion => {
                write!(f, "the source editor bridge version is unsupported")
            }
            Self::Timeout => write!(
                f,
                "the source editor did not respond; action was not completed"
            ),
            Self::Store(error) => write!(f, "{error}"),
            Self::Transition(error) => write!(f, "source editor transition failed: {error:?}"),
        }
    }
}

impl From<StoreError> for SourceSyncError {
    fn from(value: StoreError) -> Self {
        match value {
            StoreError::NoDocument => Self::NoDocument,
            StoreError::ConflictPending => Self::ConflictPending,
            error => Self::Store(error),
        }
    }
}

#[cfg(test)]
mod tests;
