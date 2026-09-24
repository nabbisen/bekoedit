//! Task 027: the editor reports a CRLF document in *editor form* (every line
//! break `\n`, which is what CodeMirror's `doc.toString()` returns), and the
//! controller accepts that text at every mode switch.

use super::*;

const CRLF_FILE: &str = "# Title\r\nsecond line\r\n";
const EDITOR_FORM: &str = "# Title\nsecond line\n";

/// A clean, saved-looking CRLF document, with the Text editor mounted on it.
fn clean_crlf_document() -> (SourceSyncState, AppState, u64) {
    let mut app = app();
    let old = app.session.take().unwrap();
    app.session = Some(bekoedit_core::DocumentSession::from_text(
        old.document_id,
        old.path,
        CRLF_FILE.into(),
    ));
    let mut sync = SourceSyncState::default();
    make_ready_as(&mut sync, &mut app, SourceEditorId::Text);
    let document_id = app.session.as_ref().unwrap().document_id;
    sync.drain_actions();
    (sync, app, document_id)
}

/// Switches to Preview, delivering the editor's snapshot `text`.
fn switch_without_editing(
    sync: &mut SourceSyncState,
    app: &mut AppState,
    document_id: u64,
    text: &str,
) {
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let SubmitOutcome::SnapshotRequested(operation_id) = sync.submit(
        SourceCommand::SwitchMode(EditorMode::Preview),
        Some(document_id),
        10,
    ) else {
        unreachable!()
    };
    sync.drain_actions();
    sync.handle_event(
        SourceEditorEvent::Snapshot {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            seq: 1,
            text: text.into(),
            composing: false,
        },
        app,
        11,
    )
    .unwrap();
}

/// Task 027 consequence 2, fixed. This test was committed first as an
/// unfixed demonstration with inverted assertions (it asserted the defect and
/// passed); the fix commit flipped it. A snapshot that is only the editor form
/// of an unedited CRLF document is not an edit: no revision, no dirty flag, and
/// not one byte of the canonical text changes.
#[test]
fn an_unedited_crlf_snapshot_changes_nothing() {
    let (mut sync, mut app, document_id) = clean_crlf_document();
    let revision_before = app.session.as_ref().unwrap().revision;
    switch_without_editing(&mut sync, &mut app, document_id, EDITOR_FORM);
    let session = app.session.as_ref().unwrap();
    assert!(!session.dirty, "no edit, so the document stays clean");
    assert_eq!(session.revision, revision_before);
    assert_eq!(session.canonical_text, CRLF_FILE);
}

/// A real edit still keeps every CRLF: only the typed text is new.
#[test]
fn an_edited_crlf_snapshot_keeps_every_other_line_ending() {
    let (mut sync, mut app, document_id) = clean_crlf_document();
    switch_without_editing(&mut sync, &mut app, document_id, "# Title\nsecond X line\n");
    let session = app.session.as_ref().unwrap();
    assert!(session.dirty);
    assert_eq!(session.canonical_text, "# Title\r\nsecond X line\r\n");
}
