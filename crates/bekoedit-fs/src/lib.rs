//! Filesystem services for bekoedit.
//!
//! Implements the Rust-owned filesystem authority required by the
//! architectural invariants (RFC-000 #3) and specified by:
//! - RFC-003: workspace model and recent workspaces
//! - RFC-004: file tree index
//! - RFC-005: safe file operations and external change detection
//! - RFC-007: atomic write, fingerprints, and recovery snapshots
//!
//! The WebView UI never receives filesystem handles; it receives
//! projections built from this crate and submits commands that are
//! validated here (path scoping, traversal rejection, name sanitization).

pub mod atomic;
pub mod ops;
pub mod paths;
pub mod recent;
pub mod recovery;
pub mod tree;
pub mod workspace;

pub use atomic::{FileFingerprint, atomic_write};
pub use ops::{
    DeleteStrategy, FileOpError, create_folder, create_markdown_file, delete_path, rename_path,
};
pub use paths::{PathError, resolve_in_workspace, sanitize_file_name};
pub use recent::{RecentWorkspaceEntry, RecentWorkspaces};
pub use recovery::{RecoverySnapshot, RecoveryStore};
pub use tree::{FileNodeKind, FileTreeIndex, FileTreeNode};
pub use workspace::{Workspace, WorkspaceError};

#[cfg(test)]
mod tests;
