// Settings and recents persistence tests — bekoedit-core.

//! Tests validating RFC-006/007/008/009 acceptance criteria: session
//! revisions and dirty state, debounced autosave, atomic save with
//! external-change protection, conflict resolution, and recovery.

use std::path::Path;

use bekoedit_fs::{DeleteStrategy, RecoveryStore};
use bekoedit_markdown::{FormBlockEdit, FormEditCommand};

use crate::conflict::{ConflictResolution, ConflictState};
use crate::save::{AutosaveScheduler, SaveState};
use crate::session::{DocumentSession, SessionError};
use crate::store::{AppState, StoreError};

fn test_state(dir: &Path) -> AppState {
    AppState::new(
        RecoveryStore::at(dir.join(".recovery")),
        dir.join(".recent.json"),
        100,
    )
}

fn workspace_with_doc(content: &str) -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("doc.md"), content).unwrap();
    let mut state = test_state(dir.path());
    state.open_workspace(dir.path(), 0).unwrap();
    state.open_document(Path::new("doc.md")).unwrap();
    (dir, state)
}

// --- session (RFC-006) ---

// ── settings persistence (survives process restart) ──────────────────────

#[test]
fn settings_persist_across_app_state_restart() {
    use bekoedit_fs::{UserSettings, load_user_settings, save_user_settings};
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.json");

    // Write settings using the fs-layer helpers.
    let mut settings = UserSettings::default();
    settings.autosave_debounce_ms = 2500;
    settings.show_hidden_files = true;
    save_user_settings(&settings_path, &settings).expect("save settings");

    // Load them in a fresh context — simulates a restart.
    let loaded = load_user_settings(&settings_path).expect("load settings");
    assert_eq!(
        loaded.autosave_debounce_ms, 2500,
        "autosave_debounce_ms persisted"
    );
    assert!(loaded.show_hidden_files, "show_hidden_files persisted");
}

#[test]
fn recent_workspaces_persist_across_restart() {
    use bekoedit_fs::RecentWorkspaces;
    let dir = tempfile::tempdir().unwrap();
    let recents_path = dir.path().join(".recent.json");

    // Record a workspace in the first "session".
    let mut state1 = AppState::new(
        RecoveryStore::at(dir.path().join(".recovery")),
        recents_path.clone(),
        100,
    );
    std::fs::write(dir.path().join("note.md"), "# Note\n").unwrap();
    state1.open_workspace(dir.path(), 0).unwrap();

    // Load recents directly from the persisted file in a fresh "session".
    let loaded_recents = RecentWorkspaces::load(&recents_path);
    assert!(
        !loaded_recents.entries.is_empty(),
        "recent workspaces must persist to disk"
    );
    assert_eq!(
        loaded_recents.entries[0].root_path,
        dir.path(),
        "most-recently-used workspace must be first"
    );
}
