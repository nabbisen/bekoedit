//! Shared contract types crossing the WebView boundary (RFC-002): the
//! bridge schema version, the editing-mode enum, and the versioned
//! `source_editor` request/event protocol. A leaf crate — depends on
//! `serde` alone, so the shell and the source editor's JavaScript side can
//! agree on wire shapes without either depending on the other's
//! implementation.

use serde::{Deserialize, Serialize};

/// Bridge schema version; bumped on incompatible payload changes.
pub const BRIDGE_SCHEMA_VERSION: u32 = 2;

pub mod source_editor;

/// Editing modes.
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
