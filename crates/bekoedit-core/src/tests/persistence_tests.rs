// Settings and recents persistence tests — bekoedit-core.

use bekoedit_fs::{RecoveryStore, UserSettings, load_user_settings, save_user_settings};

use crate::store::AppState;

// ── settings persistence (survives process restart) ──────────────────────

#[test]
fn settings_persist_across_app_state_restart() {
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.json");

    // Write settings using the fs-layer helpers.
    let settings = UserSettings {
        autosave_debounce_ms: 2500,
        show_hidden_files: true,
        ..Default::default()
    };
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
