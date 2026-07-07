//! AppState extension — store file ops.

use crate::save::SaveState;
use crate::store::{AppState, StoreError};
use bekoedit_fs::DeleteStrategy;
use std::path::{Path, PathBuf};

impl AppState {
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
}
