//! UI-side state plumbing (RFC-009): one `AppState` store behind a Dioxus
//! signal, plus the wall-clock helpers the pure core deliberately does not
//! own.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bekoedit_core::AppState;
use bekoedit_fs::RecentWorkspaces;
use dioxus::prelude::Signal;

use crate::persistence::AppPersistence;
use crate::settings::AppSettings;

// Dioxus contexts are keyed by type. Keep each independent UI flag in a
// distinct newtype so one panel cannot accidentally read or mutate another.
#[derive(Clone, Copy)]
pub struct ExplorerCollapsed(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct SettingsOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct OutlineOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct SearchOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct BacklinksOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct HistoryOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct NewFileOpen(pub Signal<bool>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMenu {
    None,
    App,
    EditorTools,
}

#[derive(Clone, Copy)]
pub struct OpenMenuState(pub Signal<OpenMenu>);

/// Autosave debounce (external design §25.4 default).
pub const AUTOSAVE_DEBOUNCE_MS: u64 = 1500;

/// Outcome of RFC-043's launch-time reopen decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchWorkspaceDecision {
    /// The most recent workspace was opened.
    Opened,
    /// Reopen was not attempted -- the setting is disabled, or there are
    /// no recent workspaces. Start Screen, no notice (RFC-043 §6).
    NotAttempted,
    /// The most recent workspace was attempted and failed. Start Screen,
    /// with a notice naming it. Never falls through to an older entry
    /// (RFC-043 §9) -- this function only ever looks at the head of
    /// `recents`.
    Failed { display_name: String },
}

/// Pure decision: given the setting, the recent-workspaces list, and a
/// way to attempt opening a path, decide what happens to the workspace
/// at launch. Testable with no filesystem and no display (RFC-043 §10):
/// inject `recents` and `try_open` rather than mutating `HOME`/
/// `XDG_CONFIG_HOME`.
///
/// `try_open` is injected so this reuses `AppState::open_workspace`'s own
/// success/failure as the usability signal in production, rather than a
/// hand-rolled duplicate check (implementation handoff §3.5).
///
/// `recents` must be the list as recorded, not `AppState::recents`:
/// `AppState::new`/`new_with_history` already prunes missing entries from
/// its own copy at construction (RFC-003, unrelated to this feature and
/// untouched by it). Reading that pruned copy here would silently
/// promote an older entry to "most recent" exactly when the true
/// most-recent one is unusable -- the fallthrough RFC-043 §9 forbids.
pub fn decide_launch_workspace(
    reopen_enabled: bool,
    recents: &RecentWorkspaces,
    mut try_open: impl FnMut(&Path) -> bool,
) -> LaunchWorkspaceDecision {
    if !reopen_enabled {
        return LaunchWorkspaceDecision::NotAttempted;
    }
    let Some(entry) = recents.entries.first() else {
        return LaunchWorkspaceDecision::NotAttempted;
    };
    if try_open(&entry.root_path) {
        LaunchWorkspaceDecision::Opened
    } else {
        LaunchWorkspaceDecision::Failed {
            display_name: entry.display_name.clone(),
        }
    }
}

/// A workspace named by `decide_launch_workspace` could not be reopened.
/// Surfaced once at startup as a non-blocking toast (RFC-043 §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenFailureNotice {
    pub display_name: String,
}

/// Builds the store with the launch-selected persistence locations, and
/// -- per RFC-043 -- opens the most recent workspace before first render
/// when the setting is enabled and it is usable. Never opens a document
/// (RFC-043 §5; recovery precedence depends on this -- opening a
/// *workspace* leaves `session` as `None`) and never falls through to an
/// older recent (§9).
pub fn create_app_state(
    persistence: &AppPersistence,
    settings: &AppSettings,
) -> (AppState, Option<ReopenFailureNotice>) {
    let mut state = persistence.create_app_state(AUTOSAVE_DEBOUNCE_MS);
    let recents = RecentWorkspaces::load(&persistence.recents_file());
    let now = now_secs();
    let decision = decide_launch_workspace(settings.reopen_last_workspace, &recents, |path| {
        state.open_workspace(path, now).is_ok()
    });
    let notice = match decision {
        LaunchWorkspaceDecision::Failed { display_name } => {
            Some(ReopenFailureNotice { display_name })
        }
        LaunchWorkspaceDecision::Opened | LaunchWorkspaceDecision::NotAttempted => None,
    };
    (state, notice)
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_secs() -> u64 {
    now_ms() / 1000
}

#[cfg(test)]
mod tests;
