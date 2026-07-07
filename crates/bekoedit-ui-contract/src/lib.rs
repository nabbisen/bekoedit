//! Versioned, serializable command and event payloads crossing the WebView
//! boundary (RFC-002).
//!
//! The UI layer sends `UiToCoreCommand` values expressing user intent; the
//! Rust core answers with `CoreToUiEvent` projections and status. Payloads
//! are compact (no whole-document strings except where RFC-011 explicitly
//! allows Text Mode snapshots), versioned, and validated on arrival.
//! Deserialization failure is a recoverable UI error, never a panic.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use bekoedit_core::{ConflictResolution, ConflictState, SaveState};
use bekoedit_fs::{DeleteStrategy, FileTreeIndex, RecentWorkspaceEntry};
use bekoedit_markdown::{FormEditCommand, HeadingNode, MarkdownDiagnostic};

/// Bridge schema version; bumped on incompatible payload changes.
pub const BRIDGE_SCHEMA_VERSION: u32 = 1;

/// Commands from the WebView UI to the Rust core (RFC-002 §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum UiToCoreCommand {
    OpenWorkspace {
        path: PathBuf,
    },
    OpenRecentWorkspace {
        path: PathBuf,
    },
    OpenDocument {
        relative_path: PathBuf,
    },
    /// Text Mode whole-document snapshot after debounce (RFC-011 MVP).
    TextSnapshot {
        document_id: u64,
        base_revision: u64,
        text: String,
    },
    /// Form Mode semantic edit; carries no authoritative byte ranges.
    FormEdit {
        document_id: u64,
        edit: FormEditCommand,
    },
    SaveNow {
        document_id: u64,
    },
    ResolveConflict {
        document_id: u64,
        resolution: ConflictResolution,
    },
    CreateMarkdownFile {
        parent: PathBuf,
        name: String,
    },
    RenamePath {
        target: PathBuf,
        new_name: String,
    },
    DeletePath {
        target: PathBuf,
        strategy: DeleteStrategy,
    },
    RefreshTree,
    SwitchMode {
        mode: EditorMode,
    },
}

/// Events from the Rust core to the WebView UI (RFC-002 §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CoreToUiEvent {
    WorkspaceOpened {
        display_name: String,
        tree: FileTreeIndex,
        recents: Vec<RecentWorkspaceEntry>,
    },
    DocumentOpened {
        document_id: u64,
        revision: u64,
        text: String,
        outline: Vec<HeadingNode>,
    },
    ProjectionUpdated {
        document_id: u64,
        revision: u64,
        diagnostics: Vec<MarkdownDiagnostic>,
    },
    SaveStatusChanged {
        document_id: u64,
        state: SaveState,
    },
    ConflictDetected {
        document_id: u64,
        state: ConflictState,
    },
    TreeUpdated {
        tree: FileTreeIndex,
    },
    ErrorRaised {
        code: String,
        message: String,
    },
}

/// Editing modes (RFC-010 as resolved 2026-06-07: Split deferred post-MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorMode {
    Text,
    Form,
    Preview,
    /// Side-by-side Text + Preview (RFC-010 Split Mode, enabled from v0.3.0).
    Split,
}

#[cfg(test)]
mod tests;
