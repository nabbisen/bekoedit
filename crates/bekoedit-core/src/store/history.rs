//! AppState extension — store history.

use crate::store::{AppState, StoreError};

impl AppState {
    // --- Local document history ---

    /// Lists save history for the current document, newest first.
    pub fn list_history(&self) -> Vec<bekoedit_fs::HistoryEntry> {
        self.session
            .as_ref()
            .map(|s| self.history.list(&s.path))
            .unwrap_or_default()
    }

    /// Restores a history entry as the current document state, creating a
    /// new dirty edit. Does not write to disk automatically.
    pub fn restore_history(
        &mut self,
        entry: &bekoedit_fs::HistoryEntry,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        session.apply_restored_snapshot(entry.text.clone());
        self.after_edit(now_ms);
        Ok(())
    }

    /// Restores a crash-recovery snapshot as a dirty edit.
    ///
    /// The stale startup snapshot is removed before the normal dirty-edit
    /// lifecycle runs; `after_edit` then writes the fresh recovery snapshot for
    /// the restored text, so the recovery channel remains protected if the app
    /// exits before save.
    pub fn restore_recovery_snapshot(
        &mut self,
        snapshot: &bekoedit_fs::RecoverySnapshot,
        now_ms: u64,
    ) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        if let Some(ws) = self.workspace.as_ref().map(|w| w.root_path.clone()) {
            let rel = snapshot
                .original_path
                .strip_prefix(&ws)
                .map_err(|e| StoreError::SaveFailed(e.to_string()))?
                .to_path_buf();
            self.open_document(&rel)?;
        }
        if self.session.is_none() {
            let id = self.allocate_document_id();
            self.session = Some(crate::session::DocumentSession::from_text(
                id,
                snapshot.original_path.clone(),
                String::new(),
            ));
            self.save_state = crate::save::SaveState::Clean;
            self.conflict = crate::conflict::ConflictState::None;
            self.autosave.resume();
            self.autosave.clear();
        }
        let session_path = self
            .session
            .as_ref()
            .ok_or(StoreError::NoDocument)?
            .path
            .clone();
        if session_path != snapshot.original_path {
            return Err(StoreError::SaveFailed(
                "recovery snapshot path does not match open document".into(),
            ));
        }
        self.recovery
            .remove(&snapshot.original_path)
            .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
        self.restore_history(
            &bekoedit_fs::HistoryEntry {
                original_path: snapshot.original_path.clone(),
                text: snapshot.text.clone(),
                saved_at_secs: snapshot.created_at_secs,
                revision: snapshot.revision,
            },
            now_ms,
        )
    }
}
