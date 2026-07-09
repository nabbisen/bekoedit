// File-delete protection tests — bekoedit-core.

//! Tests validating RFC-006/007/008/009 acceptance criteria: session
//! revisions and dirty state, debounced autosave, atomic save with
//! external-change protection, conflict resolution, and recovery.

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

// ── dirty-document delete is blocked (MVP acceptance) ────────────────────

#[test]
fn delete_dirty_document_returns_document_dirty() {
    use std::path::Path;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("note.md"), "# Note\n").unwrap();
    let mut state = test_state(root);
    state.open_workspace(root, 0).unwrap();
    state.open_document(Path::new("note.md")).unwrap();
    // Make the document dirty
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# Modified\n".into(), 100).unwrap();
    assert!(state.session.as_ref().unwrap().dirty);
    // Attempting to delete the open dirty document must be refused
    let err = state
        .delete_path(Path::new("note.md"), bekoedit_fs::DeleteStrategy::Permanent)
        .unwrap_err();
    assert!(
        matches!(err, StoreError::DocumentDirty),
        "expected DocumentDirty, got: {err:?}"
    );
}

#[test]
fn delete_clean_document_succeeds() {
    use std::path::Path;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("clean.md"), "# Clean\n").unwrap();
    let mut state = test_state(root);
    state.open_workspace(root, 0).unwrap();
    state.open_document(Path::new("clean.md")).unwrap();
    state.save_now(100).unwrap();
    // After saving, deletion must succeed
    state
        .delete_path(
            Path::new("clean.md"),
            bekoedit_fs::DeleteStrategy::Permanent,
        )
        .unwrap();
    assert!(
        state.session.is_none(),
        "session should clear after deleting open file"
    );
}
