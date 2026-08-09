//! User preferences and local configuration (RFC-022).
//!
//! `UserSettings` is a plain data struct, flattened into `bekoedit-app`'s
//! `AppSettings` via `#[serde(flatten)]` — persistence (path resolution,
//! load, save) lives there, not here. Settings affecting Markdown behavior
//! (ignored dirs) are kept here; UI-only preferences (lang, default mode)
//! live in the app crate's wrapper.

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
