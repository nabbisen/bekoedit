//! Local document history — lightweight per-document save log.
//!
//! On every successful save, `HistoryStore::record` writes a timestamped
//! snapshot alongside the recovery data. The store caps the per-document
//! history at 50 entries and prunes the oldest on overflow.
//!
//! Design principles:
//! - History is write-once (snapshots are never modified after creation).
//! - History is advisory: restoring creates a new dirty edit; it never
//!   writes to disk automatically.
//! - History lives in app-data, outside the workspace, so it cannot
//!   accidentally commit to Git.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_HISTORY: usize = 50;

/// One historical snapshot of a document's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Absolute path of the document when the snapshot was taken.
    pub original_path: PathBuf,
    /// Saved text content.
    pub text: String,
    /// Seconds since the Unix epoch at save time.
    pub saved_at_secs: u64,
    /// Document revision at the time of save.
    pub revision: u64,
}

/// A directory-backed history store keyed by document path.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    dir: PathBuf,
}

impl HistoryStore {
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn default_location() -> Self {
        let base = dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir);
        Self::at(base.join("bekoedit").join("history"))
    }

    fn doc_dir(&self, path: &Path) -> PathBuf {
        // One subdirectory per document, named by a hash of its path.
        let key = crate::atomic::FileFingerprint::of_bytes(path.to_string_lossy().as_bytes(), None)
            .content_hash;
        self.dir.join(format!("{key:016x}"))
    }

    /// Records a new snapshot. Prunes oldest entries beyond `MAX_HISTORY`.
    pub fn record(&self, entry: &HistoryEntry) -> std::io::Result<()> {
        let doc_dir = self.doc_dir(&entry.original_path);
        std::fs::create_dir_all(&doc_dir)?;
        let filename = format!("{}.json", entry.saved_at_secs);
        let json =
            serde_json::to_string(entry).map_err(|e| std::io::Error::other(e.to_string()))?;
        crate::atomic::atomic_write(&doc_dir.join(filename), &json)?;
        self.prune(&doc_dir)?;
        Ok(())
    }

    /// Returns all history entries for `path`, sorted newest-first.
    pub fn list(&self, path: &Path) -> Vec<HistoryEntry> {
        let doc_dir = self.doc_dir(path);
        if !doc_dir.exists() {
            return Vec::new();
        }
        let mut entries: Vec<HistoryEntry> = std::fs::read_dir(&doc_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let text = std::fs::read_to_string(e.path()).ok()?;
                serde_json::from_str(&text).ok()
            })
            .collect();
        entries.sort_by_key(|e: &HistoryEntry| std::cmp::Reverse(e.saved_at_secs));
        entries
    }

    fn prune(&self, doc_dir: &Path) -> std::io::Result<()> {
        let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(doc_dir)?
            .flatten()
            .filter_map(|e| {
                let stem = e
                    .path()
                    .file_stem()?
                    .to_string_lossy()
                    .parse::<u64>()
                    .ok()?;
                Some((stem, e.path()))
            })
            .collect();
        if files.len() <= MAX_HISTORY {
            return Ok(());
        }
        // Sort oldest-first and remove the excess.
        files.sort_by_key(|(t, _)| *t);
        let to_remove = files.len() - MAX_HISTORY;
        for (_, path) in files.into_iter().take(to_remove) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}
