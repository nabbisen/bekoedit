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

#[test]
fn session_tracks_revision_and_dirty() {
    let mut s = DocumentSession::from_text(1, "/x.md".into(), "# a\n".into());
    assert_eq!(s.revision, 1);
    assert!(!s.dirty);
    s.apply_text_snapshot(1, "# b\n".into()).unwrap();
    assert_eq!(s.revision, 2);
    assert!(s.dirty);
    assert_eq!(s.index.document_revision, 2);
}

#[test]
fn identical_snapshot_is_a_noop() {
    let mut s = DocumentSession::from_text(1, "/x.md".into(), "# a\n".into());
    s.apply_text_snapshot(1, "# a\n".into()).unwrap();
    assert_eq!(s.revision, 1);
    assert!(!s.dirty);
}

#[test]
fn stale_text_snapshot_is_rejected() {
    let mut s = DocumentSession::from_text(1, "/x.md".into(), "# a\n".into());
    s.apply_text_snapshot(1, "# b\n".into()).unwrap();
    let err = s.apply_text_snapshot(1, "# c\n".into()).unwrap_err();
    assert_eq!(
        err,
        SessionError::TextRevisionMismatch {
            base: 1,
            current: 2
        }
    );
    assert_eq!(s.canonical_text, "# b\n");
}

#[test]
fn form_edit_round_trip_through_session() {
    let mut s = DocumentSession::from_text(1, "/x.md".into(), "# Title\n\npara\n".into());
    let block = s.index.blocks[1].block_id;
    s.apply_form_edit(&FormEditCommand {
        base_revision: 1,
        block_id: block,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: "edited".into(),
        },
    })
    .unwrap();
    assert_eq!(s.canonical_text, "# Title\n\nedited\n");
    assert_eq!(s.revision, 2);
}

#[test]
fn invalid_utf8_is_reported_not_mangled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.md");
    std::fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
    assert_eq!(
        DocumentSession::load(1, &path).unwrap_err(),
        SessionError::NotUtf8
    );
}

// --- autosave scheduler (RFC-007) ---

#[test]
fn autosave_debounces_and_reschedules() {
    let mut a = AutosaveScheduler::new(100);
    a.note_edit(1000);
    assert!(!a.is_due(1050), "not due before debounce elapses");
    a.note_edit(1080); // further edits reschedule
    assert!(!a.is_due(1100));
    assert!(a.is_due(1180));
    a.clear();
    assert!(!a.is_due(9999));
}

#[test]
fn paused_autosave_never_fires() {
    let mut a = AutosaveScheduler::new(100);
    a.note_edit(0);
    a.pause();
    assert!(!a.is_due(10_000));
    a.resume();
    a.note_edit(10_000);
    assert!(a.is_due(10_100));
}

// --- store flows (RFC-007/008/009) ---

#[test]
fn edit_then_autosave_persists_atomically() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# v2\n".into(), 1000).unwrap();
    assert!(matches!(
        state.save_state,
        SaveState::AutoSaveScheduled { .. }
    ));
    assert!(!state.autosave_tick(1050).unwrap(), "debounce not elapsed");
    assert!(state.autosave_tick(1100).unwrap(), "due autosave writes");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap(),
        "# v2\n"
    );
    assert!(matches!(state.save_state, SaveState::Saved { .. }));
    assert!(!state.session.as_ref().unwrap().dirty);
    assert!(
        state.recovery_store().list().is_empty(),
        "snapshot cleared after save"
    );
}

#[test]
fn recovery_snapshot_exists_while_dirty() {
    let (_dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# unsaved\n".into(), 1000).unwrap();
    let snapshots = state.recovery_store().list();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].text, "# unsaved\n");
}

#[test]
fn external_change_with_dirty_memory_blocks_save() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# mine\n".into(), 0).unwrap();
    // External tool modifies the file.
    std::fs::write(dir.path().join("doc.md"), "# theirs\n").unwrap();

    assert_eq!(
        state.check_external_change(),
        ConflictState::DiskChangedDirtyMemory
    );
    assert!(
        state.autosave.is_paused(),
        "autosave pauses during conflict"
    );
    assert_eq!(state.save_now(1).unwrap_err(), StoreError::ConflictPending);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap(),
        "# theirs\n",
        "the external version must not be overwritten silently"
    );
    assert!(
        state.edit_text(99, "x".into(), 2).is_err(),
        "edits blocked during conflict"
    );
}

#[test]
fn conflict_resolution_keep_mine() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# mine\n".into(), 0).unwrap();
    std::fs::write(dir.path().join("doc.md"), "# theirs\n").unwrap();
    state.check_external_change();
    state
        .resolve_conflict(ConflictResolution::KeepMine, 5)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap(),
        "# mine\n"
    );
    assert_eq!(state.conflict, ConflictState::None);
    assert!(!state.autosave.is_paused());
}

#[test]
fn conflict_resolution_reload_disk() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# mine\n".into(), 0).unwrap();
    std::fs::write(dir.path().join("doc.md"), "# theirs\n").unwrap();
    state.check_external_change();
    state
        .resolve_conflict(ConflictResolution::ReloadDisk, 5)
        .unwrap();
    let session = state.session.as_ref().unwrap();
    assert_eq!(session.canonical_text, "# theirs\n");
    assert!(!session.dirty);
}

#[test]
fn conflict_resolution_save_copy_protects_both_versions() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# mine\n".into(), 0).unwrap();
    std::fs::write(dir.path().join("doc.md"), "# theirs\n").unwrap();
    state.check_external_change();
    state
        .resolve_conflict(
            ConflictResolution::SaveCopy {
                relative_path: "doc-copy.md".into(),
            },
            5,
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap(),
        "# theirs\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc-copy.md")).unwrap(),
        "# mine\n"
    );
}

#[test]
fn deleted_on_disk_is_detected() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    std::fs::remove_file(dir.path().join("doc.md")).unwrap();
    assert_eq!(
        state.check_external_change(),
        ConflictState::FileDeletedOnDisk
    );
}

#[test]
fn rename_updates_open_session_path() {
    let (dir, mut state) = workspace_with_doc("# v1\n");
    state
        .rename_path(Path::new("doc.md"), "renamed.md")
        .unwrap();
    assert_eq!(
        state.session.as_ref().unwrap().path,
        dir.path().canonicalize().unwrap().join("renamed.md")
    );
}

#[test]
fn dirty_open_document_cannot_be_deleted() {
    let (_dir, mut state) = workspace_with_doc("# v1\n");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# dirty\n".into(), 0).unwrap();
    assert!(
        state
            .delete_path(Path::new("doc.md"), DeleteStrategy::Permanent)
            .is_err()
    );
}

#[test]
fn clean_open_document_delete_closes_session() {
    let (_dir, mut state) = workspace_with_doc("# v1\n");
    state
        .delete_path(Path::new("doc.md"), DeleteStrategy::Permanent)
        .unwrap();
    assert!(state.session.is_none());
}

#[test]
fn opening_workspace_records_recents() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());
    state.open_workspace(dir.path(), 42).unwrap();
    assert_eq!(state.recents.entries.len(), 1);
}
