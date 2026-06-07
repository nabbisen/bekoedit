//! Crash-recovery snapshots (RFC-007).
//!
//! Snapshots of dirty documents are stored outside the workspace, in an
//! app-data directory (RFC-007 internal notes). They are removed only
//! after a confirmed save, and restoring never overwrites the original
//! file automatically (REL/19.3: recovery must not overwrite user files).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One persisted snapshot of unsaved document text (RFC-007 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub original_path: PathBuf,
    pub text: String,
    pub revision: u64,
    /// Seconds since the Unix epoch at snapshot time.
    pub created_at_secs: u64,
}

/// A directory-backed snapshot store.
#[derive(Debug, Clone)]
pub struct RecoveryStore {
    dir: PathBuf,
}

impl RecoveryStore {
    /// Store rooted at an explicit directory (used by tests and by the app
    /// with a platform config dir).
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Store at the platform-default location.
    pub fn default_location() -> Self {
        let base = dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir);
        Self::at(base.join("bekoedit").join("recovery"))
    }

    fn snapshot_file(&self, original_path: &Path) -> PathBuf {
        // One snapshot per document path; the file name is a stable hash of
        // the original path so unrelated documents never collide.
        let key = crate::atomic::FileFingerprint::of_bytes(
            original_path.to_string_lossy().as_bytes(),
            None,
        )
        .content_hash;
        self.dir.join(format!("{key:016x}.json"))
    }

    /// Persists (or replaces) the snapshot for `snapshot.original_path`.
    pub fn save(&self, snapshot: &RecoverySnapshot) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let json =
            serde_json::to_string(snapshot).map_err(|e| std::io::Error::other(e.to_string()))?;
        crate::atomic::atomic_write(&self.snapshot_file(&snapshot.original_path), &json)?;
        Ok(())
    }

    /// Removes the snapshot for a document after a confirmed save.
    pub fn remove(&self, original_path: &Path) -> std::io::Result<()> {
        let file = self.snapshot_file(original_path);
        if file.exists() {
            std::fs::remove_file(file)?;
        }
        Ok(())
    }

    /// Lists all pending snapshots (startup recovery scan, §25.7).
    pub fn list(&self) -> Vec<RecoverySnapshot> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut snapshots: Vec<RecoverySnapshot> = entries
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|json| serde_json::from_str(&json).ok())
            .collect();
        snapshots.sort_by_key(|s: &RecoverySnapshot| s.created_at_secs);
        snapshots
    }
}
