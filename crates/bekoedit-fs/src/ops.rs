//! Safe file operations (RFC-005).
//!
//! All operations take a workspace root plus a relative path and pass
//! through `resolve_in_workspace` (traversal rejection). Deletion prefers
//! the system trash (ER-006); permanent deletion requires an explicit
//! strategy choice by the caller after stronger confirmation.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{
    PathError, ensure_markdown_extension, resolve_in_workspace, sanitize_file_name,
};

/// Deletion behavior chosen by the user through the confirmation dialog
/// (external design §19.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteStrategy {
    MoveToTrash,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum FileOpError {
    #[error("{0}")]
    Path(#[from] PathError),
    #[error("a file or folder with that name already exists")]
    AlreadyExists,
    #[error("the target does not exist")]
    NotFound,
    #[error("moving to trash failed: {0}")]
    TrashFailed(String),
    #[error("filesystem error: {0}")]
    Io(String),
}

fn io(e: std::io::Error) -> FileOpError {
    FileOpError::Io(e.to_string())
}

/// Creates an empty Markdown file under `parent_rel` (workspace-relative).
/// Appends `.md` when no Markdown extension is given; never overwrites.
pub fn create_markdown_file(
    root: &Path,
    parent_rel: &Path,
    name: &str,
) -> Result<PathBuf, FileOpError> {
    let name = ensure_markdown_extension(&sanitize_file_name(name)?);
    let parent = resolve_in_workspace(root, parent_rel)?;
    let target = parent.join(&name);
    if target.exists() {
        return Err(FileOpError::AlreadyExists);
    }
    std::fs::create_dir_all(&parent).map_err(io)?;
    std::fs::write(&target, b"").map_err(io)?;
    Ok(target.strip_prefix(root).unwrap_or(&target).to_path_buf())
}

/// Creates a folder under `parent_rel` (workspace-relative).
pub fn create_folder(root: &Path, parent_rel: &Path, name: &str) -> Result<PathBuf, FileOpError> {
    let name = sanitize_file_name(name)?;
    let parent = resolve_in_workspace(root, parent_rel)?;
    let target = parent.join(&name);
    if target.exists() {
        return Err(FileOpError::AlreadyExists);
    }
    std::fs::create_dir_all(&target).map_err(io)?;
    Ok(target.strip_prefix(root).unwrap_or(&target).to_path_buf())
}

/// Renames a file or folder within its directory. Rejects collisions.
/// The caller updates any open document session path afterwards
/// (RFC-005 acceptance: rename updates the open session).
pub fn rename_path(root: &Path, target_rel: &Path, new_name: &str) -> Result<PathBuf, FileOpError> {
    let new_name = sanitize_file_name(new_name)?;
    let target = resolve_in_workspace(root, target_rel)?;
    if !target.exists() {
        return Err(FileOpError::NotFound);
    }
    let renamed = target.parent().unwrap_or(root).join(&new_name);
    if renamed.exists() {
        return Err(FileOpError::AlreadyExists);
    }
    std::fs::rename(&target, &renamed).map_err(io)?;
    Ok(renamed.strip_prefix(root).unwrap_or(&renamed).to_path_buf())
}

/// Deletes a file or folder. `MoveToTrash` uses the OS trash where
/// available and fails (rather than silently falling back to permanent
/// deletion) when it is not.
pub fn delete_path(
    root: &Path,
    target_rel: &Path,
    strategy: DeleteStrategy,
) -> Result<(), FileOpError> {
    let target = resolve_in_workspace(root, target_rel)?;
    if !target.exists() {
        return Err(FileOpError::NotFound);
    }
    match strategy {
        DeleteStrategy::MoveToTrash => {
            trash::delete(&target).map_err(|e| FileOpError::TrashFailed(e.to_string()))
        }
        DeleteStrategy::Permanent => {
            if target.is_dir() {
                std::fs::remove_dir_all(&target).map_err(io)
            } else {
                std::fs::remove_file(&target).map_err(io)
            }
        }
    }
}
