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
    DeleteStrategy, FileOpError, FileTreeIndex, RecentWorkspaces, RecoverySnapshot, RecoveryStore,
    Workspace, WorkspaceError, atomic_write,
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
    recovery: RecoveryStore,
    next_document_id: u64,
}

impl AppState {
    pub fn new(recovery: RecoveryStore, recents_file: PathBuf, autosave_debounce_ms: u64) -> Self {
        let mut recents = RecentWorkspaces::load(&recents_file);
        recents.prune_missing();
        Self {
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

    fn workspace_root(&self) -> Result<&Path, StoreError> {
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
        let id = self.next_document_id;
        self.next_document_id += 1;
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

    fn after_edit(&mut self, now_ms: u64) {
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

    // --- workspace file operations (RFC-005 pass-through with refresh) ---

    pub fn create_markdown_file(
        &mut self,
        parent: &Path,
        name: &str,
    ) -> Result<PathBuf, StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        let created = bekoedit_fs::create_markdown_file(&root, parent, name)?;
        self.refresh_tree();
        Ok(created)
    }

    pub fn rename_path(&mut self, target: &Path, new_name: &str) -> Result<PathBuf, StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        let renamed = bekoedit_fs::rename_path(&root, target, new_name)?;
        // Keep the open session pointing at the renamed file (RFC-005).
        if let Some(session) = &mut self.session
            && session.path == root.join(target)
        {
            session.path = root.join(&renamed);
        }
        self.refresh_tree();
        Ok(renamed)
    }

    /// Deletes a path; refuses to delete the open document while it has
    /// unsaved changes (ER rules / FR-FS safety).
    pub fn delete_path(
        &mut self,
        target: &Path,
        strategy: DeleteStrategy,
    ) -> Result<(), StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        if let Some(session) = &self.session
            && session.path == root.join(target)
            && session.dirty
        {
            return Err(StoreError::ConflictPending);
        }
        bekoedit_fs::delete_path(&root, target, strategy)?;
        if let Some(session) = &self.session
            && session.path == root.join(target)
        {
            self.session = None;
            self.save_state = SaveState::Clean;
        }
        self.refresh_tree();
        Ok(())
    }

    // --- RFC-035: HTML export ---

    /// Exports the current document's sanitized HTML to `path` as a
    /// self-contained HTML file. Never overwrites without an explicit call;
    /// the caller chooses the output path.
    pub fn export_html(&self, path: &std::path::Path) -> Result<(), StoreError> {
        let session = self.session.as_ref().ok_or(StoreError::NoDocument)?;
        let title = session
            .path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bekoedit export".into());
        let body_html = session.preview_html();
        let full = format!(
            r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  body {{ max-width: 780px; margin: 2rem auto; padding: 0 1.5rem;
         font-family: system-ui, sans-serif; line-height: 1.65; color: #222; }}
  pre  {{ background: #f6f6f2; padding: .75em 1em; border-radius: 6px; overflow-x: auto; }}
  code {{ font-size: .92em; }}
  blockquote {{ border-left: 3px solid #ccc; margin: 0; padding-left: 1em; color: #555; }}
  table {{ border-collapse: collapse; }} td, th {{ border: 1px solid #ddd; padding: .3em .7em; }}
</style>
</head>
<body>
{body_html}
</body>
</html>
"#,
        );
        bekoedit_fs::atomic_write(path, &full)
            .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
        Ok(())
    }
}
