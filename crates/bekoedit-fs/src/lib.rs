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
pub mod backlinks;
pub mod git_status;
pub mod history;
pub mod ops;
pub mod paths;
pub mod recent;
pub mod recovery;
pub mod search;
pub mod settings;
pub mod templates;
pub mod tree;
pub mod watcher;
pub mod workspace;

pub use atomic::{FileFingerprint, atomic_write};
pub use backlinks::{BacklinkEntry, find_backlinks};
pub use git_status::{GitStatus, git_status_map};
pub use history::{HistoryEntry, HistoryStore};
pub use ops::{
    DeleteStrategy, FileOpError, create_folder, create_markdown_file, delete_path, rename_path,
};
pub use paths::{PathError, resolve_in_workspace, sanitize_file_name};
pub use recent::{RecentWorkspaceEntry, RecentWorkspaces};
pub use recovery::{RecoverySnapshot, RecoveryStore};
pub use search::{SearchMatch, search_workspace};
pub use settings::{UserSettings, load_user_settings, save_user_settings};
pub use templates::{WorkspaceTemplate, create_from_template, list_templates};
pub use tree::{FileNodeKind, FileTreeIndex, FileTreeNode};
pub use watcher::{FsWatcher, WatchEvent};
pub use workspace::{Workspace, WorkspaceError};

#[cfg(test)]
mod tests;
