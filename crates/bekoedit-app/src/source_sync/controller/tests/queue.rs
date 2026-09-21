//! RFC-047 slice 1, §8: the queue's rules, headless. The safety rule and the
//! discard sites are in `queue_safety.rs`.

use std::path::PathBuf;

use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEpoch,
};

use super::types::{QUEUE_DEPTH, QUEUE_ENTRY_DEADLINE_MS, QueueDiscard};
use super::*;
use crate::source_sync::lifecycle::{
    DESTROY_DEADLINE_MS, MOUNT_DEADLINE_MS, PendingOperation, REFRESH_DEADLINE_MS,
    RESUME_DEADLINE_MS, SNAPSHOT_DEADLINE_MS,
};

pub(super) const DOCUMENT: u64 = 7;

pub(super) fn identity_for(document_id: u64) -> EditorIdentity {
    EditorIdentity {
        instance_id: EditorInstanceId::new(1),
        editor_id: SourceEditorId::Text,
        document_id,
        epoch: SourceEpoch::new(1),
    }
}

/// A teardown that never times out, so only the queue's own clock is tested.
pub(super) fn unmounting_sync(document_id: u64) -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = LifecycleState::Unmounting {
        retired: identity_for(document_id),
        operation: PendingOperation {
            operation_id: OperationId::new(3),
            deadline_ms: u64::MAX,
        },
        waiting: None,
    };
    sync
}

/// The page's `destroyed` event reaching Rust.
pub(super) fn deliver_destroyed(sync: &mut SourceSyncState, app: &mut AppState, now_ms: u64) {
    let LifecycleState::Unmounting {
        retired, operation, ..
    } = sync.lifecycle.state.clone()
    else {
        panic!("expected Unmounting, found {:?}", sync.lifecycle.state);
    };
    sync.handle_event(
        SourceEditorEvent::Destroyed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: operation.operation_id,
            identity: retired,
        },
        app,
        now_ms,
    )
    .unwrap();
}

pub(super) fn open(name: &str) -> SourceCommand {
    SourceCommand::OpenDocument(PathBuf::from(name))
}

fn executed(actions: &[ControllerAction]) -> Vec<SourceCommand> {
    actions
        .iter()
        .filter_map(|action| match action {
            ControllerAction::Execute { command, .. } => Some(command.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_unavailable_editor_with_a_retired_instance_is_never_queued() {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = LifecycleState::Unavailable {
        retired: Some(identity_for(DOCUMENT)),
    };

    let outcome = sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);

    assert_eq!(outcome, SubmitOutcome::Unavailable);
    assert!(sync.queue.is_empty());
    assert!(!sync.has_discards());
}

#[test]
fn commands_run_in_the_order_they_were_accepted() {
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    assert_eq!(
        sync.submit(open("a.md"), Some(DOCUMENT), 10),
        SubmitOutcome::Queued
    );
    assert_eq!(
        sync.submit(
            SourceCommand::SwitchMode(EditorMode::Form),
            Some(DOCUMENT),
            11
        ),
        SubmitOutcome::Queued
    );
    assert!(
        sync.drain_actions().is_empty(),
        "nothing runs during the teardown"
    );

    deliver_destroyed(&mut sync, &mut app, 20);

    assert_eq!(
        executed(&sync.drain_actions()),
        vec![open("a.md"), SourceCommand::SwitchMode(EditorMode::Form)]
    );
    assert!(sync.queue.is_empty());
    assert!(!sync.has_discards());
}

#[test]
fn a_newer_switch_mode_replaces_a_queued_one_and_runs_once() {
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(
        SourceCommand::SwitchMode(EditorMode::Form),
        Some(DOCUMENT),
        10,
    );
    sync.submit(
        SourceCommand::SwitchMode(EditorMode::Preview),
        Some(DOCUMENT),
        11,
    );
    assert_eq!(sync.queue.len(), 1);

    deliver_destroyed(&mut sync, &mut app, 20);

    assert_eq!(
        executed(&sync.drain_actions()),
        vec![SourceCommand::SwitchMode(EditorMode::Preview)],
        "one switch, to the newer mode; no intermediate Form"
    );
    assert!(!sync.has_discards(), "superseding is not a discard");
}

#[test]
fn a_replaced_switch_mode_keeps_its_place_in_the_order() {
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(
        SourceCommand::SwitchMode(EditorMode::Form),
        Some(DOCUMENT),
        10,
    );
    sync.submit(open("a.md"), Some(DOCUMENT), 11);
    sync.submit(
        SourceCommand::SwitchMode(EditorMode::Preview),
        Some(DOCUMENT),
        12,
    );

    let queued: Vec<_> = sync
        .queue
        .iter()
        .map(|entry| entry.command.clone())
        .collect();
    assert_eq!(
        queued,
        vec![SourceCommand::SwitchMode(EditorMode::Preview), open("a.md")]
    );
}

#[test]
fn a_newer_save_now_for_the_same_document_is_absorbed() {
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);
    assert_eq!(
        sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 11),
        SubmitOutcome::Queued
    );
    assert_eq!(sync.queue.len(), 1);
    assert!(!sync.has_discards());

    // The document the saves were for is the one still open after the teardown.
    app.session.as_mut().unwrap().document_id = DOCUMENT;
    deliver_destroyed(&mut sync, &mut app, 20);

    assert_eq!(
        executed(&sync.drain_actions()),
        vec![SourceCommand::SaveNow],
        "the two saves ran once"
    );
}

#[test]
fn a_save_now_for_another_document_is_its_own_intent_and_is_not_absorbed() {
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);
    sync.submit(SourceCommand::SaveNow, Some(DOCUMENT + 1), 11);

    assert_eq!(
        sync.queue.len(),
        2,
        "each save keeps the document it was pressed for"
    );
}

#[test]
fn a_full_queue_refuses_the_newest_and_records_it() {
    let mut sync = unmounting_sync(DOCUMENT);
    for (index, name) in ["a.md", "b.md"].into_iter().enumerate() {
        assert_eq!(
            sync.submit(open(name), Some(DOCUMENT), 10 + index as u64),
            SubmitOutcome::Queued
        );
    }
    assert_eq!(sync.queue.len(), QUEUE_DEPTH);

    let outcome = sync.submit_with_focus(open("c.md"), Some(DOCUMENT), 20, Some(9));

    assert_eq!(outcome, SubmitOutcome::QueueFull);
    let queued: Vec<_> = sync
        .queue
        .iter()
        .map(|entry| entry.command.clone())
        .collect();
    assert_eq!(
        queued,
        vec![open("a.md"), open("b.md")],
        "the oldest are kept"
    );
    assert_eq!(
        sync.drain_discards(),
        vec![QueueDiscard {
            command: open("c.md"),
            reason: DiscardReason::Overflow,
            focus_token: Some(9),
        }]
    );
}

#[test]
fn coalescing_takes_no_capacity_so_it_is_not_an_overflow() {
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit(
        SourceCommand::SwitchMode(EditorMode::Form),
        Some(DOCUMENT),
        10,
    );
    sync.submit(open("a.md"), Some(DOCUMENT), 11);
    assert_eq!(sync.queue.len(), QUEUE_DEPTH);

    let outcome = sync.submit(
        SourceCommand::SwitchMode(EditorMode::Preview),
        Some(DOCUMENT),
        12,
    );

    assert_eq!(outcome, SubmitOutcome::Queued);
    assert!(!sync.has_discards());
    assert_eq!(sync.queue.len(), QUEUE_DEPTH);
}

/// A state with no deadline of its own, so the queue's is the only bound.
fn barrier_held_sync() -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = LifecycleState::BarrierHeld {
        editor: crate::source_sync::lifecycle::HeldEditor {
            ready: crate::source_sync::lifecycle::ReadyEditor {
                identity: identity_for(DOCUMENT),
                revision: 1,
                last_seq: 0,
            },
            snapshot_operation: OperationId::new(2),
            certainty: crate::source_sync::lifecycle::HoldCertainty::Confirmed,
        },
        command: SourceCommand::SaveNow,
        before: crate::source_sync::lifecycle::SessionFingerprint {
            document_id: Some(DOCUMENT),
            revision: Some(1),
            source_token: DOCUMENT,
        },
    };
    sync
}

#[test]
fn an_entry_expires_at_the_derived_deadline_and_is_recorded() {
    let mut sync = barrier_held_sync();
    assert!(
        sync.lifecycle.next_deadline().is_none(),
        "BarrierHeld has no deadline"
    );
    let accepted_at = 100;
    assert_eq!(
        sync.submit(open("a.md"), Some(DOCUMENT), accepted_at),
        SubmitOutcome::Queued
    );
    let deadline = accepted_at + QUEUE_ENTRY_DEADLINE_MS;

    sync.tick(Some(DOCUMENT), deadline - 1).unwrap();
    assert_eq!(sync.queue.len(), 1, "one millisecond early it still waits");
    assert!(!sync.has_discards());

    sync.tick(Some(DOCUMENT), deadline).unwrap();

    assert!(sync.queue.is_empty());
    assert_eq!(
        sync.drain_discards(),
        vec![QueueDiscard {
            command: open("a.md"),
            reason: DiscardReason::Expired,
            focus_token: None,
        }]
    );
}

#[test]
fn the_entry_deadline_is_pinned_to_the_mount_deadline() {
    assert_eq!(QUEUE_ENTRY_DEADLINE_MS, MOUNT_DEADLINE_MS + 1_000);
    for lifecycle_deadline in [
        MOUNT_DEADLINE_MS,
        SNAPSHOT_DEADLINE_MS,
        RESUME_DEADLINE_MS,
        REFRESH_DEADLINE_MS,
        DESTROY_DEADLINE_MS,
    ] {
        assert!(
            QUEUE_ENTRY_DEADLINE_MS > lifecycle_deadline,
            "an entry must outlast every lifecycle deadline it can wait behind"
        );
    }
}

#[test]
fn a_command_waits_out_a_snapshot_and_runs_when_the_controller_settles_with_no_further_action() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let SubmitOutcome::SnapshotRequested(operation_id) = sync.submit(
        SourceCommand::SwitchMode(EditorMode::Preview),
        Some(document_id),
        10,
    ) else {
        unreachable!()
    };
    sync.drain_actions();
    // Form is submitted while Preview's snapshot is still in flight.
    assert_eq!(
        sync.submit(
            SourceCommand::SwitchMode(EditorMode::Form),
            Some(document_id),
            11
        ),
        SubmitOutcome::Queued
    );
    sync.handle_event(
        SourceEditorEvent::Snapshot {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            seq: 1,
            text: "typed\n".into(),
            composing: false,
        },
        &mut app,
        12,
    )
    .unwrap();
    assert_eq!(
        executed(&sync.drain_actions()),
        vec![SourceCommand::SwitchMode(EditorMode::Preview)]
    );
    assert_eq!(sync.queue.len(), 1, "Form still waits behind Preview");

    // The host runs Preview; the controller destroys the editor.
    let mut mode = EditorMode::Text;
    crate::source_sync::commands::execute(
        &mut app,
        &mut mode,
        &mut false,
        &SourceCommand::SwitchMode(EditorMode::Preview),
        13,
    )
    .unwrap();
    assert_eq!(
        sync.command_completed(true, fingerprint(&app), 14).unwrap(),
        CommandDisposition::Destroy
    );
    let ControllerAction::Lifecycle(LifecycleEffect::Destroy(retired, destroy_operation)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(sync.queue.len(), 1, "still waiting during the teardown");

    // The event that ends the teardown is the only thing that happens next.
    sync.handle_event(
        SourceEditorEvent::Destroyed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: destroy_operation,
            identity: retired,
        },
        &mut app,
        15,
    )
    .unwrap();

    assert_eq!(
        executed(&sync.drain_actions()),
        vec![SourceCommand::SwitchMode(EditorMode::Form)],
        "the queued command was submitted by that event alone"
    );
    assert!(sync.queue.is_empty());
}

#[test]
fn unmounting_the_editor_keeps_the_queue_because_a_mode_switch_does_exactly_that() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    sync.queue.push_back(super::types::QueuedCommand {
        command: SourceCommand::SwitchMode(EditorMode::Form),
        focus_token: None,
        deadline_ms: u64::MAX,
        scope: super::types::QueueScope::Anywhere,
    });
    sync.drain_actions();

    sync.force_unmount(20);

    assert_eq!(
        sync.queue.len(),
        1,
        "the host dropping the editor must not lose it"
    );
    assert!(!sync.has_discards());
    deliver_destroyed(&mut sync, &mut app, 21);
    assert_eq!(
        executed(&sync.drain_actions()),
        vec![SourceCommand::SwitchMode(EditorMode::Form)]
    );
}

#[test]
fn the_focus_token_travels_with_the_entry_into_its_execute() {
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    sync.submit_with_focus(open("a.md"), Some(DOCUMENT), 10, Some(41));

    deliver_destroyed(&mut sync, &mut app, 20);

    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Execute {
            focus_token: Some(41),
            protected: false,
            ..
        }]
    ));
}

#[test]
fn an_entry_that_became_a_no_op_while_it_waited_is_dropped_silently_and_the_next_runs() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    for command in [
        SourceCommand::SwitchMode(EditorMode::Text),
        SourceCommand::OpenSettings,
    ] {
        sync.queue.push_back(super::types::QueuedCommand {
            command,
            focus_token: None,
            deadline_ms: u64::MAX,
            scope: super::types::QueueScope::Anywhere,
        });
    }
    sync.drain_actions();

    sync.tick(Some(document_id), 20).unwrap();

    assert!(!sync.has_discards(), "a no-op is not reported");
    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Lifecycle(
            LifecycleEffect::RequestSnapshot(..)
        )]
    ));
    assert!(matches!(
        sync.lifecycle.state,
        LifecycleState::SnapshotPending {
            command: SourceCommand::OpenSettings,
            ..
        }
    ));
}
