//! External-modification conflict detection and resolution (RFC-008).
//!
//! Detection compares the session's last-known disk fingerprint with the
//! file's current state. It runs before every write (REL-001) and may be
//! triggered on demand. Neither version is ever lost silently: resolution
//! is always an explicit user choice (external design §19.4).

use std::path::Path;

use serde::{Deserialize, Serialize};

use bekoedit_fs::FileFingerprint;

/// Conflict situations (external design §24.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConflictState {
    #[default]
    None,
    /// Disk changed but memory is clean: safe to offer reload/auto-refresh.
    DiskChangedCleanMemory,
    /// Disk changed while memory has unsaved edits: user decision required.
    DiskChangedDirtyMemory,
    /// The backing file disappeared from disk.
    FileDeletedOnDisk,
}

impl ConflictState {
    pub fn requires_user_decision(&self) -> bool {
        matches!(
            self,
            ConflictState::DiskChangedDirtyMemory | ConflictState::FileDeletedOnDisk
        )
    }
}

/// The user's resolution choice (external design §19.4 conflict dialog).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Save the in-memory version over the disk version.
    KeepMine,
    /// Discard local changes and reload from disk.
    ReloadDisk,
    /// Save the in-memory version to a new workspace-relative path.
    SaveCopy { relative_path: std::path::PathBuf },
}

/// Detects the current conflict state for a document at `path` whose
/// last-known disk identity is `fingerprint`.
pub fn detect(path: &Path, fingerprint: Option<&FileFingerprint>, dirty: bool) -> ConflictState {
    let Some(fp) = fingerprint else {
        return ConflictState::None;
    };
    if !path.exists() {
        return ConflictState::FileDeletedOnDisk;
    }
    match fp.disk_changed(path) {
        Ok(true) if dirty => ConflictState::DiskChangedDirtyMemory,
        Ok(true) => ConflictState::DiskChangedCleanMemory,
        Ok(false) => ConflictState::None,
        // Read errors are treated conservatively as requiring attention.
        Err(_) => ConflictState::FileDeletedOnDisk,
    }
}
