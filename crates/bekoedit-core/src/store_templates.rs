//! AppState extension — store templates.

use crate::store::{AppState, StoreError};

impl AppState {
    // --- RFC-037: workspace templates ---

    /// Lists available workspace templates from `.bekoedit/templates/`.
    pub fn list_templates(&self) -> Vec<bekoedit_fs::WorkspaceTemplate> {
        self.workspace
            .as_ref()
            .map(|w| bekoedit_fs::list_templates(&w.root_path))
            .unwrap_or_default()
    }

    /// Creates a file from a template and opens it.
    pub fn create_from_template(
        &mut self,
        parent: &std::path::Path,
        name: &str,
        template_content: &str,
    ) -> Result<std::path::PathBuf, StoreError> {
        let root = self.workspace_root()?.to_path_buf();
        let created = bekoedit_fs::create_from_template(&root, parent, name, template_content)?;
        self.refresh_tree();
        Ok(created)
    }

    // --- RFC-036: Git status ---

    /// Returns the Git status map for the workspace (empty if not a Git repo).
    pub fn git_status(
        &self,
    ) -> std::collections::HashMap<std::path::PathBuf, bekoedit_fs::GitStatus> {
        self.workspace
            .as_ref()
            .map(|w| bekoedit_fs::git_status_map(&w.root_path))
            .unwrap_or_default()
    }
}
