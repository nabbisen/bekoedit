//! App-level settings (RFC-022): extends `bekoedit_fs::UserSettings` with
//! UI-only preferences (language, default editing mode) that the headless
//! crates don't need to know about.

use std::path::{Path, PathBuf};

use bekoedit_fs::UserSettings;
use bekoedit_ui_contract::EditorMode;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;

/// Combined app + UI settings persisted together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(flatten)]
    pub core: UserSettings,
    #[serde(default)]
    pub lang: Lang,
    #[serde(default = "default_mode")]
    pub default_mode: EditorMode,
    #[serde(default = "default_true")]
    pub reopen_last_workspace: bool,
}

fn default_mode() -> EditorMode {
    EditorMode::Form
}
fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            core: UserSettings::default(),
            lang: Lang::default(),
            default_mode: default_mode(),
            reopen_last_workspace: true,
        }
    }
}

impl AppSettings {
    pub fn settings_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("bekoedit")
            .join("app-settings.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::settings_path())
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = bekoedit_fs::atomic_write(&path, &json);
        }
    }
}
