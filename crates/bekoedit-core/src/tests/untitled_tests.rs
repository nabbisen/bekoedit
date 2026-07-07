// Untitled-file and workspace-lifecycle tests — bekoedit-core.
//
// Covers the v0.10/v0.11 AppState methods: new_untitled(), save_as(),
// and close_workspace(). These are pure logic and need no UI harness.

use std::path::Path;

use bekoedit_fs::RecoveryStore;

use crate::store::{AppState, StoreError};

fn test_state(dir: &Path) -> AppState {
    AppState::new(
        RecoveryStore::at(dir.join(".recovery")),
        dir.join(".recent.json"),
        100,
    )
}

#[test]
fn new_untitled_creates_in_memory_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());

    assert!(state.session.is_none(), "no document before new_untitled");
    state.new_untitled();

    let session = state
        .session
        .as_ref()
        .expect("document exists after new_untitled");
    assert!(session.is_untitled, "session is flagged untitled");
    assert_eq!(session.canonical_text, "", "untitled document starts empty");
}

#[test]
fn untitled_save_now_is_blocked_until_save_as() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());
    state.new_untitled();

    // save_now must refuse: the UI is expected to show a Save As dialog.
    let err = state.save_now(1000).unwrap_err();
    assert!(matches!(err, StoreError::Untitled), "got {err:?}");
}

#[test]
fn save_as_writes_to_disk_and_clears_untitled() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());
    state.new_untitled();

    let target = dir.path().join("note.md");
    state
        .save_as(target.clone(), 2000)
        .expect("save_as succeeds");

    assert!(target.exists(), "file is written to disk");
    let session = state.session.as_ref().unwrap();
    assert!(!session.is_untitled, "no longer untitled after save_as");
    assert_eq!(session.path, target, "path points at the chosen location");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "",
        "the (empty) untitled buffer is persisted verbatim"
    );
}

#[test]
fn save_as_without_document_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());
    let err = state.save_as(dir.path().join("x.md"), 1000).unwrap_err();
    assert!(matches!(err, StoreError::NoDocument), "got {err:?}");
}

#[test]
fn new_untitled_increments_document_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());

    state.new_untitled();
    let id1 = state.session.as_ref().unwrap().document_id;
    state.new_untitled();
    let id2 = state.session.as_ref().unwrap().document_id;

    assert_ne!(id1, id2, "each untitled document gets a distinct id");
}

#[test]
fn close_workspace_clears_everything() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("doc.md"), "# Doc\n").unwrap();
    let mut state = test_state(dir.path());

    state.open_workspace(dir.path(), 100).unwrap();
    state.open_document(Path::new("doc.md")).unwrap();
    assert!(state.workspace.is_some(), "workspace open");
    assert!(state.session.is_some(), "document open");

    state.close_workspace();
    assert!(state.workspace.is_none(), "workspace cleared");
    assert!(state.session.is_none(), "session cleared");
}

#[test]
fn close_workspace_does_not_touch_disk() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.md");
    std::fs::write(&file, "# original\n").unwrap();
    let mut state = test_state(dir.path());

    state.open_workspace(dir.path(), 100).unwrap();
    state.open_document(Path::new("doc.md")).unwrap();
    state.close_workspace();

    // close_workspace must not silently save dirty edits.
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "# original\n",
        "file on disk is untouched by close_workspace"
    );
}
