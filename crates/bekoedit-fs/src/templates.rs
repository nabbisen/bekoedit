//! Workspace templates (RFC-037).
//!
//! Templates are Markdown files stored in `.bekoedit/templates/` within the
//! workspace root. Creating a file from a template copies the template
//! content; the file is then opened for editing. The template directory
//! is created on first use; its absence is not an error.

use std::path::{Path, PathBuf};

/// One available template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTemplate {
    /// Display name (filename without extension).
    pub name: String,
    /// Template content read from disk.
    pub content: String,
    /// Workspace-relative path of the template file.
    pub path: PathBuf,
}

/// Lists all templates in `{root}/.bekoedit/templates/`.
/// Returns an empty list when the directory does not exist.
pub fn list_templates(root: &Path) -> Vec<WorkspaceTemplate> {
    let dir = templates_dir(root);
    if !dir.exists() {
        return Vec::new();
    }
    let mut templates: Vec<WorkspaceTemplate> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| super::paths::is_markdown_path(&e.path()))
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let content = std::fs::read_to_string(&path).ok()?;
            let rel = path.strip_prefix(root).ok()?.to_path_buf();
            Some(WorkspaceTemplate {
                name,
                content,
                path: rel,
            })
        })
        .collect();
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

/// Creates a new Markdown file at `{parent_rel}/{name}` (inside workspace)
/// pre-filled with `template_content`. The `.bekoedit/` directory is
/// created if it does not exist.
pub fn create_from_template(
    root: &Path,
    parent_rel: &Path,
    name: &str,
    template_content: &str,
) -> Result<PathBuf, super::ops::FileOpError> {
    let name = super::paths::ensure_markdown_extension(&super::paths::sanitize_file_name(name)?);
    let parent = super::paths::resolve_in_workspace(root, parent_rel)?;
    let target = parent.join(&name);
    if target.exists() {
        return Err(super::ops::FileOpError::AlreadyExists);
    }
    std::fs::create_dir_all(&parent).map_err(|e| super::ops::FileOpError::Io(e.to_string()))?;
    std::fs::write(&target, template_content.as_bytes())
        .map_err(|e| super::ops::FileOpError::Io(e.to_string()))?;
    Ok(target.strip_prefix(root).unwrap_or(&target).to_path_buf())
}

/// Creates the template directory and an example template if none exist.
pub fn ensure_templates_dir(root: &Path) -> std::io::Result<PathBuf> {
    let dir = templates_dir(root);
    std::fs::create_dir_all(&dir)?;
    let example = dir.join("blank.md");
    if !example.exists() {
        std::fs::write(&example, "# Title\n\n")?;
    }
    Ok(dir)
}

fn templates_dir(root: &Path) -> PathBuf {
    root.join(".bekoedit").join("templates")
}
