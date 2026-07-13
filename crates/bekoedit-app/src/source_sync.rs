//! Source editor synchronization barrier.
//!
//! Text/Split CodeMirror state can be newer than Rust canonical state. UI
//! commands that consume, mutate, save, or replace canonical source must pass
//! through this barrier so the active source editor stays mounted until its
//! latest snapshot is accepted or visibly rejected.

use std::path::PathBuf;

use bekoedit_core::{AppState, StoreError};
use bekoedit_fs::HistoryEntry;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::state::now_ms;

pub const SNAPSHOT_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEditorId {
    Text,
    Split,
}

impl SourceEditorId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Split => "split",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    SwitchMode(EditorMode),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSourceEditor {
    pub editor_id: SourceEditorId,
    pub mode: EditorMode,
    pub document_id: u64,
    pub epoch: u64,
    pub last_accepted_seq: u64,
    pub last_accepted_revision: u64,
    pub composing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub request_id: u64,
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub epoch: u64,
    pub requested_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBarrier {
    pub command: SourceCommand,
    pub request: SnapshotRequest,
    pub sent_to_js: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRefreshRequest {
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub revision: u64,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSnapshot {
    pub request_id: Option<u64>,
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub epoch: u64,
    pub seq: u64,
    pub text: String,
    pub composing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotBlockReason {
    CompositionActive,
    EditorUnavailable,
    IdentityMismatch,
    BridgeError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBlocked {
    pub request_id: u64,
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub epoch: u64,
    pub reason: SnapshotBlockReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    ExecuteNow(SourceCommand),
    SnapshotRequested(SnapshotRequest),
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotOutcome {
    Accepted,
    Complete(SourceCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSyncError {
    NoDocument,
    ConflictPending,
    NoActiveEditor,
    Busy,
    EditorMismatch,
    DocumentMismatch,
    EpochMismatch,
    RequestMismatch,
    StaleSeq,
    RevisionDrift,
    CompositionActive,
    EditorUnavailable,
    IdentityMismatch,
    BridgeError,
    Timeout,
    Store(StoreError),
}

pub fn submit_source_command(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) {
    let document_id = state.read().session.as_ref().map(|s| s.document_id);
    let outcome = sync.write().submit(command, document_id, now_ms());
    match outcome {
        SubmitOutcome::ExecuteNow(command) => {
            execute_source_command(sync, state, mode, toasts, command);
        }
        SubmitOutcome::SnapshotRequested(_) => {}
        SubmitOutcome::Busy => {
            let mut toasts = toasts;
            push_toast(
                &mut toasts,
                ToastKind::Warning,
                SourceSyncError::Busy.to_string(),
            );
        }
    }
}

pub fn handle_editor_snapshot(
    mut sync: Signal<SourceSyncState>,
    mut state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    snapshot: EditorSnapshot,
) {
    let result = {
        let mut app = state.write();
        sync.write().accept_snapshot(&mut app, snapshot, now_ms())
    };
    match result {
        Ok(SnapshotOutcome::Accepted) => {}
        Ok(SnapshotOutcome::Complete(command)) => {
            execute_source_command(sync, state, mode, toasts, command);
        }
        Err(err) => {
            let mut toasts = toasts;
            push_toast(&mut toasts, ToastKind::Error, err.to_string());
        }
    }
}

pub fn handle_snapshot_blocked(
    mut sync: Signal<SourceSyncState>,
    mut toasts: Signal<Vec<Toast>>,
    blocked: SnapshotBlocked,
) {
    let err = match sync.write().handle_blocked(blocked) {
        Ok(()) => return,
        Err(err) => err,
    };
    push_toast(&mut toasts, ToastKind::Error, err.to_string());
}

pub fn execute_source_command(
    mut sync: Signal<SourceSyncState>,
    mut state: Signal<AppState>,
    mut mode: Signal<EditorMode>,
    mut toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) {
    let result = match command {
        SourceCommand::SwitchMode(target) => {
            sync.write().clear_active();
            mode.set(target);
            Ok(None)
        }
        SourceCommand::SaveNow => state
            .write()
            .save_now(now_ms())
            .map(|()| Some((ToastKind::Success, "Saved".to_string()))),
        SourceCommand::SaveAs(path) => state
            .write()
            .save_as(path, now_ms())
            .map(|()| Some((ToastKind::Success, "Saved".to_string()))),
        SourceCommand::OpenDocument(path) => {
            sync.write().clear_active();
            state.write().open_document(&path).map(|()| None)
        }
        SourceCommand::NewUntitled => {
            sync.write().clear_active();
            state.write().new_untitled();
            Ok(None)
        }
        SourceCommand::OpenWorkspace(path) => {
            sync.write().clear_active();
            state.write().open_workspace(&path, now_ms()).map(|()| None)
        }
        SourceCommand::CloseWorkspace => {
            sync.write().clear_active();
            state.write().close_workspace();
            Ok(None)
        }
        SourceCommand::RestoreHistory(entry) => {
            let result = state
                .write()
                .restore_history(&entry, now_ms())
                .map(|()| Some((ToastKind::Info, "History restored".to_string())));
            if result.is_ok() {
                request_refresh_for_current_editor(sync, state);
            }
            result
        }
        SourceCommand::MoveSectionUp(idx) => {
            let result = state.write().move_section_up(idx, now_ms()).map(|()| None);
            if result.is_ok() {
                request_refresh_for_current_editor(sync, state);
            }
            result
        }
        SourceCommand::MoveSectionDown(idx) => {
            let result = state
                .write()
                .move_section_down(idx, now_ms())
                .map(|()| None);
            if result.is_ok() {
                request_refresh_for_current_editor(sync, state);
            }
            result
        }
    };

    match result {
        Ok(Some((kind, message))) => push_toast(&mut toasts, kind, message),
        Ok(None) => {}
        Err(err) => push_toast(&mut toasts, ToastKind::Error, err.to_string()),
    }
}

fn request_refresh_for_current_editor(mut sync: Signal<SourceSyncState>, state: Signal<AppState>) {
    let Some(active) = sync.read().active.clone() else {
        return;
    };
    let Some(session) = state.read().session.as_ref().map(|s| {
        (
            s.document_id,
            s.revision,
            s.document_id == active.document_id,
        )
    }) else {
        return;
    };
    let (document_id, revision, same_document) = session;
    if same_document {
        sync.write()
            .request_editor_refresh(active.editor_id, document_id, revision);
    } else {
        sync.write().clear_active();
    }
}

impl std::fmt::Display for SourceSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDocument => write!(f, "no document is open"),
            Self::ConflictPending => write!(f, "resolve the file conflict first"),
            Self::NoActiveEditor => write!(f, "no active source editor is available"),
            Self::Busy => write!(f, "another source operation is still syncing"),
            Self::EditorMismatch | Self::DocumentMismatch | Self::EpochMismatch => {
                write!(f, "the source editor changed before the operation finished")
            }
            Self::RequestMismatch => write!(f, "the source sync response was stale"),
            Self::StaleSeq => write!(f, "the source editor sent an old snapshot"),
            Self::RevisionDrift => write!(f, "the document changed before source sync finished"),
            Self::CompositionActive => write!(f, "finish composing text before this action"),
            Self::EditorUnavailable => write!(f, "the source editor is unavailable"),
            Self::IdentityMismatch => write!(f, "the source editor identity did not match"),
            Self::BridgeError => write!(f, "the source editor bridge failed"),
            Self::Timeout => write!(
                f,
                "the source editor did not respond; action was not completed"
            ),
            Self::Store(err) => write!(f, "{err}"),
        }
    }
}

impl From<StoreError> for SourceSyncError {
    fn from(value: StoreError) -> Self {
        match value {
            StoreError::NoDocument => Self::NoDocument,
            StoreError::ConflictPending => Self::ConflictPending,
            err => Self::Store(err),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceSyncState {
    pub active: Option<ActiveSourceEditor>,
    pub pending: Option<PendingBarrier>,
    pub refresh: Option<EditorRefreshRequest>,
    next_epoch: u64,
    next_request_id: u64,
}

impl Default for SourceSyncState {
    fn default() -> Self {
        Self {
            active: None,
            pending: None,
            refresh: None,
            next_epoch: 1,
            next_request_id: 1,
        }
    }
}

impl SourceSyncState {
    pub fn register_editor(
        &mut self,
        editor_id: SourceEditorId,
        mode: EditorMode,
        document_id: u64,
        revision: u64,
    ) -> ActiveSourceEditor {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        let active = ActiveSourceEditor {
            editor_id,
            mode,
            document_id,
            epoch,
            last_accepted_seq: 0,
            last_accepted_revision: revision,
            composing: false,
        };
        self.active = Some(active.clone());
        active
    }

    pub fn clear_active(&mut self) {
        self.active = None;
        self.pending = None;
    }

    pub fn submit(
        &mut self,
        command: SourceCommand,
        current_document_id: Option<u64>,
        now_ms: u64,
    ) -> SubmitOutcome {
        if self.pending.is_some() {
            return SubmitOutcome::Busy;
        }
        let Some(active) = self.active.as_ref() else {
            return SubmitOutcome::ExecuteNow(command);
        };
        if current_document_id != Some(active.document_id) {
            self.clear_active();
            return SubmitOutcome::ExecuteNow(command);
        }
        let request = SnapshotRequest {
            request_id: self.next_request_id,
            editor_id: active.editor_id,
            document_id: active.document_id,
            epoch: active.epoch,
            requested_at_ms: now_ms,
        };
        self.next_request_id += 1;
        self.pending = Some(PendingBarrier {
            command,
            request: request.clone(),
            sent_to_js: false,
        });
        SubmitOutcome::SnapshotRequested(request)
    }

    pub fn unsent_request_for(&self, editor_id: SourceEditorId) -> Option<SnapshotRequest> {
        let pending = self.pending.as_ref()?;
        if pending.sent_to_js || pending.request.editor_id != editor_id {
            return None;
        }
        Some(pending.request.clone())
    }

    pub fn mark_request_sent(&mut self, request_id: u64) {
        if let Some(pending) = self.pending.as_mut()
            && pending.request.request_id == request_id
        {
            pending.sent_to_js = true;
        }
    }

    pub fn expire_pending(&mut self, now_ms: u64) -> Option<SourceCommand> {
        let should_expire = self.pending.as_ref().is_some_and(|pending| {
            now_ms.saturating_sub(pending.request.requested_at_ms) >= SNAPSHOT_TIMEOUT_MS
        });
        if should_expire {
            return self.pending.take().map(|pending| pending.command);
        }
        None
    }

    pub fn request_editor_refresh(
        &mut self,
        editor_id: SourceEditorId,
        document_id: u64,
        revision: u64,
    ) -> EditorRefreshRequest {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        let request = EditorRefreshRequest {
            editor_id,
            document_id,
            revision,
            epoch,
        };
        self.active = Some(ActiveSourceEditor {
            editor_id,
            mode: match editor_id {
                SourceEditorId::Text => EditorMode::Text,
                SourceEditorId::Split => EditorMode::Split,
            },
            document_id,
            epoch,
            last_accepted_seq: 0,
            last_accepted_revision: revision,
            composing: false,
        });
        self.refresh = Some(request.clone());
        request
    }

    pub fn pending_refresh_for(&self, editor_id: SourceEditorId) -> Option<EditorRefreshRequest> {
        let request = self.refresh.as_ref()?;
        if request.editor_id == editor_id {
            Some(request.clone())
        } else {
            None
        }
    }

    pub fn clear_refresh(&mut self, editor_id: SourceEditorId, epoch: u64) {
        if self
            .refresh
            .as_ref()
            .is_some_and(|request| request.editor_id == editor_id && request.epoch == epoch)
        {
            self.refresh = None;
        }
    }

    pub fn handle_blocked(&mut self, blocked: SnapshotBlocked) -> Result<(), SourceSyncError> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(SourceSyncError::RequestMismatch)?;
        if pending.request.request_id != blocked.request_id {
            return Err(SourceSyncError::RequestMismatch);
        }
        validate_identity(
            self.active.as_ref(),
            blocked.editor_id,
            blocked.document_id,
            blocked.epoch,
        )?;
        self.pending = None;
        Err(match blocked.reason {
            SnapshotBlockReason::CompositionActive => SourceSyncError::CompositionActive,
            SnapshotBlockReason::EditorUnavailable => SourceSyncError::EditorUnavailable,
            SnapshotBlockReason::IdentityMismatch => SourceSyncError::IdentityMismatch,
            SnapshotBlockReason::BridgeError => SourceSyncError::BridgeError,
        })
    }

    pub fn accept_snapshot(
        &mut self,
        app: &mut AppState,
        snapshot: EditorSnapshot,
        now_ms: u64,
    ) -> Result<SnapshotOutcome, SourceSyncError> {
        if snapshot.composing {
            self.pending = None;
            return Err(SourceSyncError::CompositionActive);
        }
        let active = match validate_identity(
            self.active.as_ref(),
            snapshot.editor_id,
            snapshot.document_id,
            snapshot.epoch,
        ) {
            Ok(active) => active,
            Err(err) => {
                self.pending = None;
                return Err(err);
            }
        };
        let completing_request = match snapshot.request_id {
            Some(request_id) => {
                let Some(pending) = self.pending.as_ref() else {
                    return Err(SourceSyncError::RequestMismatch);
                };
                if pending.request.request_id != request_id {
                    self.pending = None;
                    return Err(SourceSyncError::RequestMismatch);
                }
                true
            }
            None => false,
        };
        if snapshot.seq <= active.last_accepted_seq && !completing_request {
            self.pending = None;
            return Err(SourceSyncError::StaleSeq);
        }
        let session = app.session.as_ref().ok_or(SourceSyncError::NoDocument)?;
        if session.document_id != snapshot.document_id {
            self.pending = None;
            return Err(SourceSyncError::DocumentMismatch);
        }
        if session.revision != active.last_accepted_revision {
            self.pending = None;
            return Err(SourceSyncError::RevisionDrift);
        }
        let mut new_revision = active.last_accepted_revision;
        if session.canonical_text != snapshot.text {
            app.edit_text(active.last_accepted_revision, snapshot.text, now_ms)?;
            new_revision = app
                .session
                .as_ref()
                .map(|s| s.revision)
                .ok_or(SourceSyncError::NoDocument)?;
        }
        let active = self
            .active
            .as_mut()
            .ok_or(SourceSyncError::NoActiveEditor)?;
        active.last_accepted_seq = snapshot.seq;
        active.last_accepted_revision = new_revision;
        active.composing = false;
        if completing_request {
            let pending = self
                .pending
                .take()
                .ok_or(SourceSyncError::RequestMismatch)?;
            Ok(SnapshotOutcome::Complete(pending.command))
        } else {
            Ok(SnapshotOutcome::Accepted)
        }
    }
}

fn validate_identity(
    active: Option<&ActiveSourceEditor>,
    editor_id: SourceEditorId,
    document_id: u64,
    epoch: u64,
) -> Result<&ActiveSourceEditor, SourceSyncError> {
    let active = active.ok_or(SourceSyncError::NoActiveEditor)?;
    if active.editor_id != editor_id {
        return Err(SourceSyncError::EditorMismatch);
    }
    if active.document_id != document_id {
        return Err(SourceSyncError::DocumentMismatch);
    }
    if active.epoch != epoch {
        return Err(SourceSyncError::EpochMismatch);
    }
    Ok(active)
}
