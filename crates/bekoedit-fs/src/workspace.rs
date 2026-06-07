//! Workspace opening and validation (RFC-003).
//!
//! MVP supports one active single-root workspace. Multi-root semantics
//! are out of scope (RFC-003 non-goals).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Workspace open failures, surfaced without crashing (RFC-003 acceptance).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum WorkspaceError {
    #[error("workspace path does not exist")]
    Missing,
    #[error("workspace path is not a directory")]
    NotADirectory,
    #[error("workspace path is not readable: {0}")]
    Unreadable(String),
}

/// The active single-root workspace (RFC-003 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub root_path: PathBuf,
    pub display_name: String,
}

impl Workspace {
    /// Validates and opens `root` as the active workspace.
    pub fn open(root: &Path) -> Result<Self, WorkspaceError> {
        if !root.exists() {
            return Err(WorkspaceError::Missing);
        }
        if !root.is_dir() {
            return Err(WorkspaceError::NotADirectory);
        }
        std::fs::read_dir(root).map_err(|e| WorkspaceError::Unreadable(e.to_string()))?;
        // Canonicalize where safe, but keep a user-friendly display name
        // (RFC-003 internal notes).
        let root_path = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let display_name = root_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root_path.to_string_lossy().into_owned());
        Ok(Self {
            root_path,
            display_name,
        })
    }
}
