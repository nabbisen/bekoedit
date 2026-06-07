//! Workspace-scoped path validation (requirements §17.4, SEC-001/002).
//!
//! Every UI-originated path is resolved through these functions before any
//! filesystem mutation. Path traversal outside the workspace root is
//! rejected; single-name inputs may not contain separators.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Path validation failures, mapped to user-facing errors by the app layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum PathError {
    #[error("path escapes the workspace root")]
    OutsideWorkspace,
    #[error("path contains a parent-directory component")]
    ParentTraversal,
    #[error("absolute paths are not allowed here")]
    AbsoluteNotAllowed,
    #[error("file name is empty")]
    EmptyName,
    #[error("file name contains a path separator or reserved character")]
    InvalidName,
}

/// Resolves a workspace-relative path against `root`, rejecting traversal.
///
/// The result is lexically guaranteed to be inside `root`; callers that
/// follow symlinks must additionally verify the canonical target
/// (conservative symlink policy, SEC-003).
pub fn resolve_in_workspace(root: &Path, relative: &Path) -> Result<PathBuf, PathError> {
    if relative.is_absolute() {
        return Err(PathError::AbsoluteNotAllowed);
    }
    let mut resolved = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err(PathError::ParentTraversal),
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::AbsoluteNotAllowed);
            }
        }
    }
    if !resolved.starts_with(root) {
        return Err(PathError::OutsideWorkspace);
    }
    Ok(resolved)
}

/// Validates a single file or folder name from a UI input field
/// (RFC-005 internal notes: reject separators in single-name inputs).
pub fn sanitize_file_name(name: &str) -> Result<String, PathError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(PathError::EmptyName);
    }
    if trimmed == "." || trimmed == ".." {
        return Err(PathError::InvalidName);
    }
    const FORBIDDEN: &[char] = &['/', '\\', '\0', ':', '*', '?', '"', '<', '>', '|'];
    if trimmed.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Err(PathError::InvalidName);
    }
    Ok(trimmed.to_string())
}

/// Appends `.md` when the name has no supported Markdown extension
/// (external design §19.1 New File rules).
pub fn ensure_markdown_extension(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".md") || lower.ends_with(".markdown") {
        name.to_string()
    } else {
        format!("{name}.md")
    }
}

/// Whether a path has a supported Markdown extension (requirements §13.2).
pub fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()),
        Some(ref e) if e == "md" || e == "markdown"
    )
}
