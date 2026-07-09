//! AppState extension — store sections.

use crate::save::SaveState;
use crate::store::{AppState, StoreError};

impl AppState {
    // --- RFC-029: outline section operations ---

    /// Moves the section headed by `heading_idx` one position upward among
    /// its siblings in the current document (RFC-029).
    pub fn move_section_up(&mut self, heading_idx: usize, now_ms: u64) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        let result = bekoedit_markdown::move_section_up(
            &session.canonical_text,
            &session.index,
            heading_idx,
        )
        .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
        session.apply_text_snapshot(session.revision, result.text)?;
        self.after_edit_internal(now_ms);
        Ok(())
    }

    /// Moves the section headed by `heading_idx` one position downward among
    /// its siblings (RFC-029).
    pub fn move_section_down(&mut self, heading_idx: usize, now_ms: u64) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        let result = bekoedit_markdown::move_section_down(
            &session.canonical_text,
            &session.index,
            heading_idx,
        )
        .map_err(|e| StoreError::SaveFailed(e.to_string()))?;
        session.apply_text_snapshot(session.revision, result.text)?;
        self.after_edit_internal(now_ms);
        Ok(())
    }

    fn after_edit_internal(&mut self, now_ms: u64) {
        self.autosave.note_edit(now_ms);
        self.save_state = match self.autosave.due_at() {
            Some(due) => SaveState::AutoSaveScheduled { due_at_ms: due },
            None => SaveState::Dirty,
        };
        if let Some(session) = &self.session {
            let _ = self.recovery.save(&bekoedit_fs::RecoverySnapshot {
                original_path: session.path.clone(),
                text: session.canonical_text.clone(),
                revision: session.revision,
                created_at_secs: now_ms / 1000,
            });
        }
    }
}
