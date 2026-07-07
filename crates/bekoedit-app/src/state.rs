//! UI-side state plumbing (RFC-009): one `AppState` store behind a Dioxus
//! signal, plus the wall-clock helpers the pure core deliberately does not
//! own.

use std::time::{SystemTime, UNIX_EPOCH};

use bekoedit_core::AppState;
use bekoedit_fs::{RecentWorkspaces, RecoveryStore};

/// Autosave debounce (external design §25.4 default).
pub const AUTOSAVE_DEBOUNCE_MS: u64 = 1500;

/// Builds the store with platform-default persistence locations.
pub fn create_app_state() -> AppState {
    AppState::new(
        RecoveryStore::default_location(),
        RecentWorkspaces::default_file(),
        AUTOSAVE_DEBOUNCE_MS,
    )
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[allow(dead_code)]
pub fn now_secs() -> u64 {
    now_ms() / 1000
}
