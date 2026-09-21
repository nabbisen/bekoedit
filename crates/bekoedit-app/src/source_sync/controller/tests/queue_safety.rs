//! RFC-047 slice 1, §8.6 and §8.7: a queued command never runs against a
//! document it was not meant for, and every way out of the queue that is not
//! a run leaves a record.

use std::path::Path;

use bekoedit_ui_contract::source_editor::{BridgeFailureReason, OperationId};

use super::queue::{DOCUMENT, deliver_destroyed, identity_for, open, unmounting_sync};
use super::types::QueueDiscard;
use super::*;
use crate::source_sync::lifecycle::PendingOperation;

/// A workspace with `a.md` and `b.md`, `a.md` open. The files are real, so a
/// stray save is visible on disk.
struct Workspace {
    app: AppState,
    dir: std::path::PathBuf,
}

impl Workspace {
    fn open_a() -> Self {
        let dir = tempfile::tempdir().unwrap().keep();
        std::fs::write(dir.join("a.md"), "# a\n").unwrap();
        std::fs::write(dir.join("b.md"), "# b\n").unwrap();
        let mut app = AppState::new(
            RecoveryStore::at(dir.join(".recovery")),
            dir.join(".recent.json"),
            100,
        );
        app.open_workspace(&dir, 1).unwrap();
        app.open_document(Path::new("a.md")).unwrap();
        Self { app, dir }
    }

    fn document_id(&self) -> u64 {
        self.app.session.as_ref().unwrap().document_id
    }

    fn type_into_open_document(&mut self, text: &str) {
        let revision = self.app.session.as_ref().unwrap().revision;
        self.app.edit_text(revision, text.to_string(), 5).unwrap();
    }

    fn on_disk(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap()
    }

    /// What the host does with every `Execute` the controller emitted.
    fn run(&mut self, actions: Vec<ControllerAction>) -> usize {
        let mut ran = 0;
        for action in actions {
            if let ControllerAction::Execute { command, .. } = action {
                let mut mode = EditorMode::Preview;
                crate::source_sync::commands::execute(
                    &mut self.app,
                    &mut mode,
                    &mut false,
                    &command,
                    50,
                )
                .unwrap();
                ran += 1;
            }
        }
        ran
    }
}

/// RFC-047 §5.4, the safety rule. Ctrl+S was pressed while `a.md` was open, the
/// editor was tearing down, and `b.md` was opened before it finished.
#[test]
fn a_queued_save_for_document_a_is_discarded_when_b_is_open_and_b_is_not_written() {
    let mut workspace = Workspace::open_a();
    let document_a = workspace.document_id();
    let mut sync = unmounting_sync(document_a);
    assert_eq!(
        sync.submit(SourceCommand::SaveNow, Some(document_a), 10),
        SubmitOutcome::Queued
    );

    // The document changes while the entry waits, and B holds unsaved text.
    workspace.app.open_document(Path::new("b.md")).unwrap();
    let document_b = workspace.document_id();
    assert_ne!(document_a, document_b);
    workspace.type_into_open_document("# b, edited and never saved\n");

    deliver_destroyed(&mut sync, &mut workspace.app, 20);

    // What did NOT happen: no save was emitted, and so none ran.
    let actions = sync.drain_actions();
    assert!(
        !actions.iter().any(|action| matches!(
            action,
            ControllerAction::Execute {
                command: SourceCommand::SaveNow,
                ..
            }
        )),
        "a save recorded for document A must not be emitted while B is open: {actions:?}"
    );
    assert_eq!(workspace.run(actions), 0);
    assert_eq!(workspace.on_disk("b.md"), "# b\n", "B was not written");
    assert_eq!(
        workspace.on_disk("a.md"),
        "# a\n",
        "A was not written either"
    );
    assert!(
        workspace
            .app
            .session
            .as_ref()
            .unwrap()
            .canonical_text
            .contains("never saved"),
        "B's unsaved text is still unsaved in memory"
    );

    // What did happen: the discard is recorded, naming the command and why.
    assert_eq!(
        sync.drain_discards(),
        vec![QueueDiscard {
            command: SourceCommand::SaveNow,
            reason: DiscardReason::DocumentChanged,
            focus_token: None,
        }]
    );
    assert!(sync.queue.is_empty());
}

/// The control for the test above: with the document unchanged the same entry
/// does run, and the write is visible on disk. It shows the test could see a
/// stray save.
#[test]
fn a_queued_save_whose_document_is_still_open_runs_and_writes_it() {
    let mut workspace = Workspace::open_a();
    let document_a = workspace.document_id();
    let mut sync = unmounting_sync(document_a);
    sync.submit(SourceCommand::SaveNow, Some(document_a), 10);
    workspace.type_into_open_document("# a, edited\n");

    deliver_destroyed(&mut sync, &mut workspace.app, 20);

    assert_eq!(workspace.run(sync.drain_actions()), 1);
    assert_eq!(workspace.on_disk("a.md"), "# a, edited\n");
    assert!(!sync.has_discards());
}

#[test]
fn every_document_scoped_command_is_validated_and_the_others_are_not() {
    let scoped = [
        SourceCommand::SaveNow,
        SourceCommand::SaveAs(std::path::PathBuf::from("x.md")),
        SourceCommand::MoveSectionUp(0),
        SourceCommand::MoveSectionDown(0),
        SourceCommand::RestoreHistory(bekoedit_fs::HistoryEntry {
            original_path: std::path::PathBuf::from("a.md"),
            text: "old".into(),
            saved_at_secs: 1,
            revision: 1,
        }),
    ];
    let unscoped = [
        SourceCommand::SwitchMode(EditorMode::Form),
        open("a.md"),
        SourceCommand::OpenSettings,
        SourceCommand::NewUntitled,
        SourceCommand::OpenWorkspace(std::path::PathBuf::from("w")),
        SourceCommand::CloseWorkspace,
    ];
    let mut app = app();
    for (command, discarded) in scoped
        .into_iter()
        .map(|command| (command, true))
        .chain(unscoped.into_iter().map(|command| (command, false)))
    {
        let mut sync = unmounting_sync(DOCUMENT);
        sync.submit(command.clone(), Some(DOCUMENT), 10);
        // Another document is open when the teardown ends.
        app.session.as_mut().unwrap().document_id = DOCUMENT + 1;
        deliver_destroyed(&mut sync, &mut app, 20);
        assert_eq!(
            sync.has_discards(),
            discarded,
            "{command:?}: discarded = {discarded} when the open document changed"
        );
    }
}

#[test]
fn a_document_scoped_entry_waits_behind_a_command_the_host_has_not_run_yet() {
    let mut workspace = Workspace::open_a();
    let document_a = workspace.document_id();
    let mut sync = unmounting_sync(document_a);
    sync.submit(open("b.md"), Some(document_a), 10);
    sync.submit(SourceCommand::SaveNow, Some(document_a), 11);

    deliver_destroyed(&mut sync, &mut workspace.app, 20);

    // Opening b.md is emitted; the save cannot be judged until it has run.
    let first = sync.drain_actions();
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(
        sync.queue.len(),
        1,
        "the save is held, not run and not lost"
    );
    assert!(!sync.has_discards());

    // The host runs it: B is open now, with unsaved text. The save was for A.
    let mut mode = EditorMode::Preview;
    crate::source_sync::commands::execute(
        &mut workspace.app,
        &mut mode,
        &mut false,
        &open("b.md"),
        30,
    )
    .unwrap();
    workspace.type_into_open_document("# b, edited and never saved\n");
    sync.tick(Some(workspace.document_id()), 31).unwrap();

    assert_eq!(workspace.run(sync.drain_actions()), 0);
    assert_eq!(workspace.on_disk("b.md"), "# b\n", "B was not written");
    assert_eq!(
        sync.drain_discards(),
        vec![QueueDiscard {
            command: SourceCommand::SaveNow,
            reason: DiscardReason::DocumentChanged,
            focus_token: None,
        }]
    );
}

fn record(command: SourceCommand, reason: DiscardReason) -> Vec<QueueDiscard> {
    vec![QueueDiscard {
        command,
        reason,
        focus_token: None,
    }]
}

/// §8.7: the sites that used to assign `None` to the waiting slot.

#[test]
fn shutdown_records_what_it_empties() {
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(open("a.md"), Some(DOCUMENT), 10);

    sync.shutdown(20);

    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        record(open("a.md"), DiscardReason::Shutdown)
    );
}

#[test]
fn losing_the_relay_records_what_it_empties() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    sync.relay_generation_started(1);
    assert!(sync.relay_generation_ready(1, 10));
    assert!(matches!(
        sync.submit(SourceCommand::SaveNow, Some(document_id), 11),
        SubmitOutcome::SnapshotRequested(_)
    ));
    assert_eq!(
        sync.submit(SourceCommand::OpenSettings, Some(document_id), 12),
        SubmitOutcome::Queued
    );

    assert!(sync.relay_disconnected(1));

    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        record(SourceCommand::OpenSettings, DiscardReason::RelayLost)
    );
}

#[test]
fn a_timeout_that_leaves_the_editor_unavailable_records_what_it_empties() {
    let mut sync = unmounting_sync(DOCUMENT);
    sync.lifecycle.state = LifecycleState::Unmounting {
        retired: identity_for(DOCUMENT),
        operation: PendingOperation {
            operation_id: OperationId::new(3),
            deadline_ms: 100,
        },
        waiting: None,
    };
    sync.submit(open("a.md"), Some(DOCUMENT), 10);

    let outcome = sync.tick(Some(DOCUMENT), 200).unwrap();

    assert_eq!(outcome, TickOutcome::TimedOut);
    assert!(sync.is_unavailable());
    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        record(open("a.md"), DiscardReason::EditorUnavailable)
    );
}

#[test]
fn an_event_that_leaves_the_editor_unavailable_records_what_it_empties() {
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(open("a.md"), Some(DOCUMENT), 10);

    let result = sync.handle_event(
        SourceEditorEvent::DestroyFailed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: OperationId::new(3),
            identity: identity_for(DOCUMENT),
            reason: BridgeFailureReason::BridgeError,
        },
        &mut app,
        20,
    );

    assert!(
        result.is_err(),
        "the failure itself is still reported as an error"
    );
    assert!(sync.is_unavailable());
    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        record(open("a.md"), DiscardReason::EditorUnavailable)
    );
}

#[test]
fn an_editor_that_fails_to_initialise_records_the_command_waiting_for_it() {
    let mut app = app();
    let document_id = app.session.as_ref().unwrap().document_id;
    let identity = identity_for(document_id);
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = LifecycleState::Initializing {
        identity,
        revision: app.session.as_ref().unwrap().revision,
        operation: PendingOperation {
            operation_id: OperationId::new(4),
            deadline_ms: u64::MAX,
        },
    };
    assert_eq!(
        sync.submit(
            SourceCommand::SwitchMode(EditorMode::Form),
            Some(document_id),
            10
        ),
        SubmitOutcome::WaitingForReady
    );

    let result = sync.handle_event(
        SourceEditorEvent::InitFailed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: OperationId::new(4),
            identity,
            reason: BridgeFailureReason::BridgeError,
        },
        &mut app,
        20,
    );

    assert!(result.is_err());
    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        record(
            SourceCommand::SwitchMode(EditorMode::Form),
            DiscardReason::EditorUnavailable
        )
    );
}
