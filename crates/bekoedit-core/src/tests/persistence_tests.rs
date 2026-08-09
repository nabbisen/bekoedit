// Recents persistence tests — bekoedit-core.

use bekoedit_fs::RecoveryStore;

use crate::store::AppState;

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
    let expected_root = dir.path().canonicalize().unwrap();
    assert_eq!(
        loaded_recents.entries[0].root_path, expected_root,
        "most-recently-used workspace must be first"
    );
}
