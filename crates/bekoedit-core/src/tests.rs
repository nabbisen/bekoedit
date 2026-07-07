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

mod delete_tests;
mod persistence_tests;
mod session_tests;
