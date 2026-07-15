//! Application-root source-editor lifecycle and synchronization controller.

use std::path::PathBuf;

use bekoedit_core::{AppState, StoreError};
use bekoedit_fs::HistoryEntry;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::state::now_ms;

mod commands;
mod controller;
mod focus;
pub mod host;
pub mod lifecycle;

pub use bekoedit_ui_contract::source_editor::SourceEditorId;
pub use controller::{EditorMountHandle, MountOutcome, SourceSyncState, SubmitOutcome};
pub use focus::{
    SourceInteractionOrigin, cancel_source_focus, submit_source_interaction,
    submit_source_shortcut_interaction,
};
pub use lifecycle::MountIntent;

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

pub fn submit_source_command(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    _mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) -> SubmitOutcome {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        focus::cancel_focus_guards_through(token);
    }
    submit_source_command_preserving_focus(sync, state, _mode, toasts, command, None)
}

fn submit_source_command_preserving_focus(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    _mode: Signal<EditorMode>,
    mut toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    focus_token: Option<u64>,
) -> SubmitOutcome {
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
        | SubmitOutcome::WaitingForReady => {}
        SubmitOutcome::Busy => {
            crate::bridge::trace("source.controller.busy", "");
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
