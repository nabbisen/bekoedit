//! The application state store (RFC-009).
//!
//! Owns the workspace, file tree, the single open document session (MVP
//! resolution of requirements Open Question 10), the save lifecycle, and
//! conflict state. All UI commands flow through these methods, which apply
//! the strict data lifecycle of RFC-000 §9: validate, resolve, mutate,
//! rebuild projections.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use bekoedit_fs::{
    FileOpError, FileTreeIndex, RecentWorkspaces, RecoverySnapshot, RecoveryStore, Workspace,
    WorkspaceError, atomic_write,
};
use bekoedit_markdown::FormEditCommand;

use crate::conflict::{self, ConflictResolution, ConflictState};
use crate::save::{AutosaveScheduler, SaveState};
use crate::session::{DocumentSession, SessionError};

/// Store-level command failures (mapped to user-facing errors by the GUI).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum StoreError {
    #[error("no workspace is open")]
    NoWorkspace,
    #[error("no document is open")]
    NoDocument,
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    FileOp(#[from] FileOpError),
    #[error("the file changed on disk; resolve the conflict first")]
    ConflictPending,
    #[error("document has unsaved changes; save or discard before this operation")]
    DocumentDirty,
    #[error("this is a new untitled file; use Save As to choose a location")]
    Untitled,
    #[error("save failed: {0}")]
    SaveFailed(String),
}

/// Root application state (RFC-009 §7).
pub struct AppState {
    pub workspace: Option<Workspace>,
    pub tree: FileTreeIndex,
    pub session: Option<DocumentSession>,
    pub save_state: SaveState,
    pub conflict: ConflictState,
    pub autosave: AutosaveScheduler,
    pub recents: RecentWorkspaces,
    recents_file: PathBuf,
    pub recovery: RecoveryStore,
    pub(crate) history: bekoedit_fs::HistoryStore,
    next_document_id: u64,
}

impl AppState {
    pub fn new(recovery: RecoveryStore, recents_file: PathBuf, autosave_debounce_ms: u64) -> Self {
        let mut recents = RecentWorkspaces::load(&recents_file);
        recents.prune_missing();
        Self {
            history: bekoedit_fs::HistoryStore::default_location(),
            workspace: None,
            tree: FileTreeIndex::default(),
            session: None,
            save_state: SaveState::Clean,
            conflict: ConflictState::None,
            autosave: AutosaveScheduler::new(autosave_debounce_ms),
            recents,
            recents_file,
            recovery,
            next_document_id: 1,
        }
    }

    pub fn recovery_store(&self) -> &RecoveryStore {
        &self.recovery
    }

    pub(crate) fn allocate_document_id(&mut self) -> u64 {
        let id = self.next_document_id;
        self.next_document_id += 1;
        id
    }

    /// Opens a folder as the active workspace and records it as recent.
    pub fn open_workspace(&mut self, root: &Path, now_secs: u64) -> Result<(), StoreError> {
        let workspace = Workspace::open(root)?;
        self.recents.record(
            workspace.root_path.clone(),
            workspace.display_name.clone(),
            now_secs,
        );
        let _ = self.recents.save(&self.recents_file);
        self.tree = FileTreeIndex::scan(&workspace.root_path, &[]);
        self.workspace = Some(workspace);
        self.session = None;
        self.save_state = SaveState::Clean;
        self.conflict = ConflictState::None;
        Ok(())
    }

    pub fn refresh_tree(&mut self) {
        if let Some(ws) = &self.workspace {
            self.tree = FileTreeIndex::scan(&ws.root_path, &[]);
        }
    }

    pub(crate) fn workspace_root(&self) -> Result<&Path, StoreError> {
        self.workspace
            .as_ref()
            .map(|w| w.root_path.as_path())
            .ok_or(StoreError::NoWorkspace)
    }

    /// Opens a workspace-relative Markdown file as the active document.
    pub fn open_document(&mut self, relative: &Path) -> Result<(), StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        let absolute =
            bekoedit_fs::resolve_in_workspace(&root, relative).map_err(FileOpError::Path)?;
        let id = self.allocate_document_id();
        let session = DocumentSession::load(id, &absolute)?;
        self.session = Some(session);
        self.save_state = SaveState::Clean;
        self.conflict = ConflictState::None;
        self.autosave.resume();
        self.autosave.clear();
        Ok(())
    }

    fn session_mut(&mut self) -> Result<&mut DocumentSession, StoreError> {
        self.session.as_mut().ok_or(StoreError::NoDocument)
    }

    pub(crate) fn after_edit(&mut self, now_ms: u64) {
        self.autosave.note_edit(now_ms);
        self.save_state = match self.autosave.due_at() {
            Some(due_at_ms) => SaveState::AutoSaveScheduled { due_at_ms },
            None => SaveState::Dirty,
        };
        if let Some(session) = &self.session {
            let _ = self.recovery.save(&RecoverySnapshot {
                original_path: session.path.clone(),
                text: session.canonical_text.clone(),
                revision: session.revision,
                created_at_secs: now_ms / 1000,
            });
        }
    }

    /// Text Mode edit (whole-document snapshot after debounce, RFC-011).
    pub fn edit_text(
        &mut self,
        base_revision: u64,
        text: String,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        self.session_mut()?
            .apply_text_snapshot(base_revision, text)?;
        self.after_edit(now_ms);
        Ok(())
    }

    /// Form Mode semantic edit (RFC-018).
    pub fn edit_form(&mut self, cmd: &FormEditCommand, now_ms: u64) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        self.session_mut()?.apply_form_edit(cmd)?;
        self.after_edit(now_ms);
        Ok(())
    }

    /// Detects external modification of the open document (RFC-008).
    pub fn check_external_change(&mut self) -> ConflictState {
        if let Some(session) = &self.session {
            self.conflict = conflict::detect(
                &session.path,
                session.disk_fingerprint.as_ref(),
                session.dirty,
            );
            if self.conflict.requires_user_decision() {
                self.autosave.pause();
                self.save_state = SaveState::ConflictResolutionRequired;
            }
        }
        self.conflict
    }

    /// Runs a due autosave. Returns true when a save happened.
    pub fn autosave_tick(&mut self, now_ms: u64) -> Result<bool, StoreError> {
        if self.session.is_none() || !self.autosave.is_due(now_ms) {
            return Ok(false);
        }
        self.save_now(now_ms)?;
        Ok(true)
    }

    /// Manual or autosave write: conflict check, atomic write, fingerprint
    /// update, recovery cleanup (RFC-007 §9 / external design §25.5).
    pub fn save_now(&mut self, now_ms: u64) -> Result<(), StoreError> {
        if self.check_external_change().requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        // Untitled files must use save_as() instead.
        if self
            .session
            .as_ref()
            .map(|s| s.is_untitled)
            .unwrap_or(false)
        {
            return Err(StoreError::Untitled);
        }
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        if !session.dirty {
            self.autosave.clear();
            return Ok(());
        }
        self.save_state = SaveState::Saving;
        match atomic_write(&session.path, &session.canonical_text) {
            Ok(fingerprint) => {
                session.mark_saved(fingerprint);
                let _ = self.recovery.remove(&session.path);
                let _ = self.history.record(&bekoedit_fs::HistoryEntry {
                    original_path: session.path.clone(),
                    text: session.canonical_text.clone(),
                    revision: session.revision,
                    saved_at_secs: now_ms / 1000,
                });
                self.autosave.clear();
                self.save_state = SaveState::Saved { at_ms: now_ms };
                Ok(())
            }
            Err(e) => {
                // Save failures keep the dirty text intact (REL-003).
                self.save_state = SaveState::SaveFailed {
                    message: e.to_string(),
                    retryable: true,
                };
                Err(StoreError::SaveFailed(e.to_string()))
            }
        }
    }

    /// Applies the user's conflict decision (external design §19.4).
    pub fn resolve_conflict(
        &mut self,
        resolution: ConflictResolution,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        match resolution {
            ConflictResolution::KeepMine => {
                let fingerprint = atomic_write(&session.path, &session.canonical_text)
                    .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
                session.mark_saved(fingerprint);
                let _ = self.recovery.remove(&session.path);
                let _ = self.history.record(&bekoedit_fs::HistoryEntry {
                    original_path: session.path.clone(),
                    text: session.canonical_text.clone(),
                    revision: session.revision,
                    saved_at_secs: now_ms / 1000,
                });
                self.save_state = SaveState::Saved { at_ms: now_ms };
            }
            ConflictResolution::ReloadDisk => {
                let id = session.document_id;
                let path = session.path.clone();
                *session = DocumentSession::load(id, &path)?;
                let _ = self.recovery.remove(&path);
                self.save_state = SaveState::Clean;
            }
            ConflictResolution::SaveCopy { relative_path } => {
                let target = bekoedit_fs::resolve_in_workspace(&root, &relative_path)
                    .map_err(FileOpError::Path)?;
                let fingerprint = atomic_write(&target, &session.canonical_text)
                    .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
                let _ = self.recovery.remove(&session.path);
                let _ = self.history.record(&bekoedit_fs::HistoryEntry {
                    original_path: session.path.clone(),
                    text: session.canonical_text.clone(),
                    revision: session.revision,
                    saved_at_secs: now_ms / 1000,
                });
                session.path = target;
                session.mark_saved(fingerprint);
                self.save_state = SaveState::Saved { at_ms: now_ms };
                self.refresh_tree();
            }
        }
        self.conflict = ConflictState::None;
        self.autosave.resume();
        Ok(())
    }
    /// Returns the size in bytes of a workspace-relative path without opening it.
    /// The UI uses this to warn users before opening very large files.
    pub fn file_size_bytes(&self, relative: &Path) -> Option<u64> {
        let root = self.workspace_root().ok()?;
        let absolute = bekoedit_fs::resolve_in_workspace(root, relative).ok()?;
        std::fs::metadata(&absolute).ok().map(|m| m.len())
    }
    // ── Untitled file support ────────────────────────────────────────────────

    /// Creates a blank in-memory document without requiring a workspace.
    /// Returns `StoreError::Untitled` from `save_now()` so the UI knows
    /// to show a "Save As" dialog.
    pub fn new_untitled(&mut self) {
        let id = self.allocate_document_id();
        self.session = Some(DocumentSession::new_untitled(id));
        self.save_state = SaveState::Dirty;
        self.conflict = crate::conflict::ConflictState::None;
        self.autosave.clear();
        self.autosave.pause(); // Don't auto-write untitled files to temp dir
    }

    /// Moves an in-memory untitled document to a permanent path and saves.
    pub fn save_as(&mut self, new_path: std::path::PathBuf, now_ms: u64) -> Result<(), StoreError> {
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        session.path = new_path.clone();
        session.is_untitled = false;
        // Write using the normal save path
        self.save_now(now_ms)
    }
    /// Closes the current workspace and clears the session, returning to the
    /// start screen. Dirty documents are not saved automatically.
    pub fn close_workspace(&mut self) {
        self.workspace = None;
        self.tree = bekoedit_fs::FileTreeIndex::default();
        self.session = None;
        self.save_state = crate::save::SaveState::Clean;
        self.conflict = crate::conflict::ConflictState::None;
        self.autosave.clear();
    }
}
