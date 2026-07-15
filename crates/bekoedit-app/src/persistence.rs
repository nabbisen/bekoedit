use std::path::{Path, PathBuf};

use bekoedit_core::AppState;
use bekoedit_fs::{HistoryStore, RecentWorkspaces, RecoveryStore};

use crate::settings::AppSettings;

#[derive(Debug, Clone)]
pub enum AppPersistence {
    PlatformDefault,
    Isolated(IsolatedPersistencePaths),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedPersistencePaths {
    root: PathBuf,
    settings_file: PathBuf,
    recents_file: PathBuf,
    recovery_dir: PathBuf,
    history_dir: PathBuf,
}

impl AppPersistence {
    pub fn platform_default() -> Self {
        Self::PlatformDefault
    }

    pub fn isolated(root: PathBuf) -> Self {
        Self::Isolated(IsolatedPersistencePaths {
            settings_file: root.join("config").join("app-settings.json"),
            recents_file: root.join("config").join("recent-workspaces.json"),
            recovery_dir: root.join("data").join("recovery"),
            history_dir: root.join("data").join("history"),
            root,
        })
    }

    pub fn load_settings(&self) -> AppSettings {
        match self {
            Self::PlatformDefault => AppSettings::load(),
            Self::Isolated(paths) => AppSettings::load_from(&paths.settings_file),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) {
        match self {
            Self::PlatformDefault => settings.save(),
            Self::Isolated(paths) => settings.save_to(&paths.settings_file),
        }
    }

    pub fn create_app_state(&self, autosave_debounce_ms: u64) -> AppState {
        match self {
            Self::PlatformDefault => AppState::new(
                RecoveryStore::default_location(),
                RecentWorkspaces::default_file(),
                autosave_debounce_ms,
            ),
            Self::Isolated(paths) => AppState::new_with_history(
                RecoveryStore::at(paths.recovery_dir.clone()),
                paths.recents_file.clone(),
                HistoryStore::at(paths.history_dir.clone()),
                autosave_debounce_ms,
            ),
        }
    }

    pub fn isolated_paths(&self) -> Option<&IsolatedPersistencePaths> {
        match self {
            Self::PlatformDefault => None,
            Self::Isolated(paths) => Some(paths),
        }
    }
}

impl IsolatedPersistencePaths {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_file(&self) -> &Path {
        &self.settings_file
    }

    pub fn recents_file(&self) -> &Path {
        &self.recents_file
    }

    pub fn recovery_dir(&self) -> &Path {
        &self.recovery_dir
    }

    pub fn history_dir(&self) -> &Path {
        &self.history_dir
    }

    pub fn all_within_root(&self) -> bool {
        [
            self.settings_file(),
            self.recents_file(),
            self.recovery_dir(),
            self.history_dir(),
        ]
        .into_iter()
        .all(|path| path.starts_with(self.root()))
    }
}

#[cfg(test)]
mod tests;
