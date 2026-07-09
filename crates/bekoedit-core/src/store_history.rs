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
    pub fn restore_history(&mut self, entry: &bekoedit_fs::HistoryEntry) -> Result<(), StoreError> {
        if self.conflict.requires_user_decision() {
            return Err(StoreError::ConflictPending);
        }
        let session = self.session.as_mut().ok_or(StoreError::NoDocument)?;
        // Use the next revision as base so any current edit is superseded.
        session.apply_text_snapshot(session.revision, entry.text.clone())?;
        Ok(())
    }
}
