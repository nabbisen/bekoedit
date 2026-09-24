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

/// Switches to Preview with no edit, delivering the editor's own snapshot.
fn switch_without_editing(sync: &mut SourceSyncState, app: &mut AppState, document_id: u64) {
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
            text: EDITOR_FORM.into(),
            composing: false,
        },
        app,
        11,
    )
    .unwrap();
}

/// DEMONSTRATION of task 027 consequence 2, on the unfixed code, with an
/// **inverted assertion** (it asserts the defect, so it passes today): a
/// snapshot that is only the editor form of an unedited CRLF document marks
/// it dirty and advances its revision. The fix commit flips these assertions.
#[test]
fn unfixed_demonstration_an_unedited_crlf_snapshot_dirties_the_document() {
    let (mut sync, mut app, document_id) = clean_crlf_document();
    assert!(!app.session.as_ref().unwrap().dirty);
    let revision_before = app.session.as_ref().unwrap().revision;
    switch_without_editing(&mut sync, &mut app, document_id);
    let session = app.session.as_ref().unwrap();
    assert!(
        session.dirty,
        "the defect: the document is dirty with no edit"
    );
    assert_eq!(session.revision, revision_before + 1);
    assert_eq!(
        session.canonical_text, EDITOR_FORM,
        "the defect: every CR is gone from the canonical text"
    );
}
