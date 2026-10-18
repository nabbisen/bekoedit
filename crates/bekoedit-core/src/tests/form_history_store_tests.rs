//! RFC-049 slice 1: the history's lifetime and gates through the store.

use std::path::Path;

use bekoedit_fs::RecoveryStore;

use super::form_history_tests::{MIXED, paragraph_edit};
use crate::conflict::ConflictResolution;
use crate::save::SaveState;
use crate::session::SessionError;
use crate::store::{AppState, StoreError};

fn store_with(files: &[(&str, &str)]) -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    let mut state = AppState::new(
        RecoveryStore::at(dir.path().join(".recovery")),
        dir.path().join(".recent.json"),
        100,
    );
    state.open_workspace(dir.path(), 0).unwrap();
    state.open_document(Path::new(files[0].0)).unwrap();
    (dir, state)
}

fn form_edit(state: &mut AppState, text: &str) {
    let s = state.session.as_ref().unwrap();
    let c = paragraph_edit(s, text);
    state.edit_form(&c, 0).unwrap();
}

fn text(state: &AppState) -> String {
    state.session.as_ref().unwrap().canonical_text.clone()
}

#[test]
fn opening_another_document_and_coming_back_leaves_empty_stacks() {
    let (_dir, mut state) = store_with(&[("a.md", MIXED), ("b.md", MIXED)]);
    form_edit(&mut state, "edited");
    state.open_document(Path::new("b.md")).unwrap();
    state.open_document(Path::new("a.md")).unwrap();
    assert!(!state.undo_form(1).unwrap());
    assert!(!state.redo_form(1).unwrap());
}

#[test]
fn a_reloaded_document_has_empty_stacks_and_undo_restores_the_reloaded_bytes() {
    let (dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "mine");
    let theirs = MIXED.replace("Last para", "Their last para");
    std::fs::write(dir.path().join("doc.md"), &theirs).unwrap();
    state.check_external_change();
    state
        .resolve_conflict(ConflictResolution::ReloadDisk, 5)
        .unwrap();

    assert!(
        !state.undo_form(6).unwrap(),
        "the old history went with the old session"
    );
    assert_eq!(text(&state), theirs);
    form_edit(&mut state, "after reload");
    assert!(state.undo_form(7).unwrap());
    assert_eq!(text(&state), theirs, "the reloaded bytes, not the old ones");
}

#[test]
fn a_revision_that_climbs_back_after_a_reload_cannot_apply_a_stale_record() {
    let (dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "mine"); // revision 2, a record stamped 2
    std::fs::write(dir.path().join("doc.md"), "# theirs\n\nbody\n").unwrap();
    state.check_external_change();
    state
        .resolve_conflict(ConflictResolution::ReloadDisk, 5)
        .unwrap();
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# typed\n\nbody\n".into(), 6).unwrap(); // revision 2 again
    let typed = text(&state);

    assert!(
        !state.undo_form(7).unwrap(),
        "no record survived the reload"
    );
    assert_eq!(text(&state), typed);
}

#[test]
fn a_save_keeps_the_stacks_and_undo_after_it_makes_the_document_dirty() {
    let (dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "edited");
    state.save_now(10).unwrap();
    assert!(!state.session.as_ref().unwrap().dirty);
    assert!(state.undo_form(11).unwrap());
    assert!(state.session.as_ref().unwrap().dirty);
    assert_eq!(text(&state), MIXED);
    assert!(!matches!(state.save_state, SaveState::Saved { .. }));
    state.save_now(12).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap(),
        MIXED
    );
}

#[test]
fn undo_and_redo_are_refused_while_a_conflict_needs_a_decision() {
    let (dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "edited");
    std::fs::write(dir.path().join("doc.md"), "# theirs\n").unwrap();
    state.check_external_change();
    assert_eq!(state.undo_form(1).unwrap_err(), StoreError::ConflictPending);
    assert_eq!(state.redo_form(1).unwrap_err(), StoreError::ConflictPending);
    assert!(
        state.session.as_ref().unwrap().can_undo_form(),
        "refused, not cleared"
    );
}

#[test]
fn a_stale_undo_through_the_store_is_an_error_and_schedules_nothing() {
    let (_dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "edited");
    let rev = state.session.as_ref().unwrap().revision;
    state.edit_text(rev, "# t\n".into(), 0).unwrap();
    state.save_state = SaveState::Clean;
    assert_eq!(
        state.undo_form(1).unwrap_err(),
        StoreError::Session(SessionError::FormHistoryStale)
    );
    assert_eq!(state.save_state, SaveState::Clean);
}

#[test]
fn a_successful_undo_goes_through_after_edit() {
    let (_dir, mut state) = store_with(&[("doc.md", MIXED)]);
    form_edit(&mut state, "edited");
    state.save_state = SaveState::Clean;
    assert!(state.undo_form(1).unwrap());
    assert_ne!(state.save_state, SaveState::Clean);
}

#[test]
fn no_error_names_the_document_text() {
    let message = SessionError::FormHistoryStale.to_string();
    assert!(!message.contains("Para"), "{message}");
}
