//! User preferences and local configuration (RFC-022).
//!
//! Settings are stored outside the workspace in the platform config directory
//! so they don't pollute the user's Markdown files. A corrupt file degrades
//! gracefully to defaults. Settings affecting Markdown behavior (ignored dirs)
//! are kept here; UI-only preferences (lang, default mode) live in the app
//! crate's thin wrapper.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Persistent user preferences (RFC-022 §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    /// Directories to exclude from the file tree, in addition to the
    /// built-in list (`.git`, `node_modules`, etc.).
    #[serde(default)]
    pub extra_ignored_dirs: Vec<String>,
    /// Show hidden files (dot-prefixed) in the explorer.
    #[serde(default)]
    pub show_hidden_files: bool,
    /// Autosave debounce in milliseconds.
    #[serde(default = "default_debounce")]
    pub autosave_debounce_ms: u64,
    /// Warn before opening files larger than this many bytes (0 = off).
    #[serde(default = "default_large_file")]
    pub large_file_warn_bytes: u64,
    /// Move deleted files to trash (true) or delete permanently (false).
    #[serde(default = "default_true")]
    pub prefer_trash: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            extra_ignored_dirs: Vec::new(),
            show_hidden_files: false,
            autosave_debounce_ms: default_debounce(),
            large_file_warn_bytes: default_large_file(),
            prefer_trash: true,
        }
    }
}

fn default_debounce() -> u64 {
    1_500
}
fn default_large_file() -> u64 {
    2 * 1024 * 1024
} // 2 MB
fn default_true() -> bool {
    true
}

impl UserSettings {
    /// Platform default settings file location.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("bekoedit")
            .join("settings.json")
    }

    /// Loads from `path`; corrupt or missing files return defaults.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Persists atomically (RFC-007 atomic write used here too).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        crate::atomic::atomic_write(path, &json)?;
        Ok(())
    }
}

/// Saves user settings to a JSON file atomically.
pub fn save_user_settings(path: &std::path::Path, settings: &UserSettings) -> std::io::Result<()> {
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| std::io::Error::other(e.to_string()))?;
    crate::atomic_write(path, &json).map(|_| ())
}

/// Loads user settings from a JSON file. Returns `Ok(Default)` if the
/// file does not exist or is corrupt (graceful degradation).
pub fn load_user_settings(path: &std::path::Path) -> std::io::Result<UserSettings> {
    if !path.exists() {
        return Ok(UserSettings::default());
    }
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| std::io::Error::other(e.to_string()))
}
