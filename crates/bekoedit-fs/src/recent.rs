//! Recent workspace persistence (RFC-003).
//!
//! Recent workspaces are local app configuration (never written into the
//! user's Markdown folders). A corrupt file degrades to an empty list and
//! never blocks app launch (RFC-022 acceptance).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 10;

/// One remembered workspace root (RFC-003 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub root_path: PathBuf,
    pub display_name: String,
    /// Seconds since the Unix epoch at last open.
    pub last_opened_at_secs: u64,
}

/// Most-recent-first workspace list with JSON persistence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaces {
    pub entries: Vec<RecentWorkspaceEntry>,
}

impl RecentWorkspaces {
    /// Platform-default storage location.
    pub fn default_file() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
        base.join("bekoedit").join("recent-workspaces.json")
    }

    /// Loads the list; missing or corrupt files yield an empty list.
    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        crate::atomic::atomic_write(file, &json)?;
        Ok(())
    }

    /// Records `root_path` as most recent, deduplicating and capping length.
    pub fn record(&mut self, root_path: PathBuf, display_name: String, now_secs: u64) {
        self.entries.retain(|e| e.root_path != root_path);
        self.entries.insert(
            0,
            RecentWorkspaceEntry {
                root_path,
                display_name,
                last_opened_at_secs: now_secs,
            },
        );
        self.entries.truncate(MAX_RECENT);
    }

    /// Drops entries whose paths no longer exist (requirements §13.7).
    pub fn prune_missing(&mut self) {
        self.entries.retain(|e| e.root_path.exists());
    }
}
