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

/// A resolved settings-file location, and whether resolving it required
/// falling back to the temp directory because the platform config
/// directory could not be determined (task 005 Part A). The temp
/// directory does not reliably survive a reboot, so callers should
/// surface `used_temp_fallback` to the user rather than silently
/// discarding it, as the previous `unwrap_or_else` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPathResolution {
    pub path: PathBuf,
    pub used_temp_fallback: bool,
}

/// Pure decision: given what `dirs::config_dir()` returned, where do
/// settings live and was that the fallback? Kept separate from
/// `AppSettings::resolve_settings_path` so the fallback is testable by
/// injecting `config_dir` rather than mutating process-wide environment
/// variables (task 005 §5).
fn resolve_settings_path_from(config_dir: Option<PathBuf>) -> SettingsPathResolution {
    match config_dir {
        Some(dir) => SettingsPathResolution {
            path: dir.join("bekoedit").join("app-settings.json"),
            used_temp_fallback: false,
        },
        None => SettingsPathResolution {
            path: std::env::temp_dir()
                .join("bekoedit")
                .join("app-settings.json"),
            used_temp_fallback: true,
        },
    }
}

impl AppSettings {
    pub fn resolve_settings_path() -> SettingsPathResolution {
        resolve_settings_path_from(dirs::config_dir())
    }

    pub fn settings_path() -> PathBuf {
        Self::resolve_settings_path().path
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
        self.save_to(&Self::settings_path());
    }

    pub fn save_to(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = bekoedit_fs::atomic_write(path, &json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_platform_config_directory_when_available() {
        let resolution = resolve_settings_path_from(Some(PathBuf::from("/home/user/.config")));
        assert!(!resolution.used_temp_fallback);
        assert_eq!(
            resolution.path,
            PathBuf::from("/home/user/.config/bekoedit/app-settings.json")
        );
    }

    #[test]
    fn falls_back_to_the_temp_directory_and_reports_it_when_unavailable() {
        let resolution = resolve_settings_path_from(None);
        assert!(resolution.used_temp_fallback);
        assert_eq!(
            resolution.path,
            std::env::temp_dir()
                .join("bekoedit")
                .join("app-settings.json")
        );
    }
}
