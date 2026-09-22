//! RFC-047 slice 1 review §3: a switch is a no-op only when it names the mode
//! the app is already heading for -- a queued switch, else one in flight, else
//! the mounted editor -- not merely the mounted editor.

use super::queue::{DOCUMENT, deliver_destroyed, unmounting_sync};
use super::types::{QueueScope, QueuedCommand};
use super::*;

fn switch(mode: EditorMode) -> SourceCommand {
    SourceCommand::SwitchMode(mode)
}

fn ready_in(editor_id: SourceEditorId) -> (SourceSyncState, AppState, u64) {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready_as(&mut sync, &mut app, editor_id);
    let document_id = app.session.as_ref().unwrap().document_id;
    sync.drain_actions();
    (sync, app, document_id)
}

fn queued_commands(sync: &SourceSyncState) -> Vec<SourceCommand> {
    sync.queue
        .iter()
        .map(|entry| entry.command.clone())
        .collect()
}

fn host_executes(app: &mut AppState, mode: &mut EditorMode, actions: &[ControllerAction]) {
    for action in actions {
        if let ControllerAction::Execute { command, .. } = action {
            crate::source_sync::commands::execute(app, mode, &mut false, command, 50).unwrap();
        }
    }
}

#[test]
fn text_clicked_while_preview_is_in_flight_is_queued_and_the_app_ends_in_text() {
    let (mut sync, mut app, document_id) = ready_in(SourceEditorId::Text);
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let SubmitOutcome::SnapshotRequested(operation_id) =
        sync.submit(switch(EditorMode::Preview), Some(document_id), 10)
    else {
        unreachable!()
    };
    sync.drain_actions();

    // Text is still mounted, and the old rule called this a no-op.
    assert_eq!(
        sync.submit(switch(EditorMode::Text), Some(document_id), 11),
        SubmitOutcome::Queued
    );
    assert_eq!(queued_commands(&sync), vec![switch(EditorMode::Text)]);

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
    let mut mode = EditorMode::Text;
    host_executes(&mut app, &mut mode, &sync.drain_actions());
    assert_eq!(
        mode,
        EditorMode::Preview,
        "Preview runs first, as it was asked first"
    );
    assert_eq!(
        sync.command_completed(true, fingerprint(&app), 13).unwrap(),
        CommandDisposition::Destroy
    );
    let ControllerAction::Lifecycle(LifecycleEffect::Destroy(retired, destroy_operation)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    sync.handle_event(
        SourceEditorEvent::Destroyed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: destroy_operation,
            identity: retired,
        },
        &mut app,
        14,
    )
    .unwrap();

    host_executes(&mut app, &mut mode, &sync.drain_actions());
    assert_eq!(mode, EditorMode::Text, "the user's last click wins");
    assert!(!sync.has_discards());
}

#[test]
fn text_clicked_while_a_preview_switch_is_queued_replaces_it_and_the_app_stays_in_text() {
    let (mut sync, mut app, document_id) = ready_in(SourceEditorId::Text);
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    // A save is in flight; Preview waits behind it.
    let SubmitOutcome::SnapshotRequested(operation_id) =
        sync.submit(SourceCommand::SaveNow, Some(document_id), 10)
    else {
        unreachable!()
    };
    sync.drain_actions();
    assert_eq!(
        sync.submit(switch(EditorMode::Preview), Some(document_id), 11),
        SubmitOutcome::Queued
    );

    // Text is mounted, and a Preview switch is queued: Text is not a no-op.
    assert_eq!(
        sync.submit(switch(EditorMode::Text), Some(document_id), 12),
        SubmitOutcome::Queued
    );
    assert_eq!(
        queued_commands(&sync),
        vec![switch(EditorMode::Text)],
        "the newer switch replaced the queued Preview"
    );

    // The save fails (an untitled document), so the editor resumes and the queue drains.
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
        13,
    )
    .unwrap();
    let mut mode = EditorMode::Text;
    let actions = sync.drain_actions();
    assert!(
        matches!(
            actions.as_slice(),
            [ControllerAction::Execute {
                command: SourceCommand::SaveNow,
                protected: true,
                ..
            }]
        ),
        "{actions:?}"
    );
    sync.command_completed(false, fingerprint(&app), 14)
        .unwrap();
    let ControllerAction::Lifecycle(LifecycleEffect::Resume(_, _, resume_operation)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    sync.handle_event(
        SourceEditorEvent::EditingResumed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: resume_operation,
            identity,
            snapshot_operation_id: operation_id,
            revision: app.session.as_ref().unwrap().revision,
            was_held: true,
        },
        &mut app,
        15,
    )
    .unwrap();

    let after = sync.drain_actions();
    host_executes(&mut app, &mut mode, &after);
    assert_eq!(mode, EditorMode::Text, "Preview never ran");
    assert!(
        !after.iter().any(|action| matches!(
            action,
            ControllerAction::Execute {
                command: SourceCommand::SwitchMode(EditorMode::Preview),
                ..
            }
        )),
        "{after:?}"
    );
    assert!(sync.queue.is_empty());
    assert!(!sync.has_discards(), "Text was already the mode: silent");
}

#[test]
fn text_clicked_while_a_switch_to_text_is_in_flight_stays_a_silent_no_op() {
    let (mut sync, _app, document_id) = ready_in(SourceEditorId::Split);
    assert!(matches!(
        sync.submit(switch(EditorMode::Text), Some(document_id), 10),
        SubmitOutcome::SnapshotRequested(_)
    ));

    assert_eq!(
        sync.submit(switch(EditorMode::Text), Some(document_id), 11),
        SubmitOutcome::NoOp
    );
    assert!(sync.queue.is_empty());
    assert!(!sync.has_discards());
}

#[test]
fn text_clicked_again_while_a_switch_to_text_is_queued_replaces_it_and_keeps_the_newer_token() {
    // Task 022 case B: this used to be a silent no-op, but `focus.rs` had
    // already allocated a fresh claim for the second click and cancelled it
    // on NoOp -- so the switch ran, but with no claim at all. Coalescing
    // instead, exactly as a different target already did, keeps the newer
    // click's token.
    let mut app = app();
    let mut sync = unmounting_sync(DOCUMENT);
    assert_eq!(
        sync.submit_with_focus(switch(EditorMode::Text), Some(DOCUMENT), 10, Some(1)),
        SubmitOutcome::Queued
    );

    let outcome = sync.submit_with_focus(switch(EditorMode::Text), Some(DOCUMENT), 11, Some(2));

    assert_eq!(
        outcome,
        SubmitOutcome::Queued,
        "not a no-op: the claim would otherwise be lost"
    );
    assert_eq!(sync.queue.len(), 1, "coalesced into one entry");
    assert_eq!(queued_commands(&sync), vec![switch(EditorMode::Text)]);
    assert!(!sync.has_discards());

    deliver_destroyed(&mut sync, &mut app, 20);

    assert!(
        matches!(
            sync.drain_actions().as_slice(),
            [ControllerAction::Execute {
                command: SourceCommand::SwitchMode(EditorMode::Text),
                focus_token: Some(2),
                ..
            }]
        ),
        "exactly one switch runs, carrying the newer token"
    );
}

#[test]
fn text_clicked_with_text_mounted_and_nothing_in_flight_stays_a_no_op() {
    for (editor_id, mode) in [
        (SourceEditorId::Text, EditorMode::Text),
        (SourceEditorId::Split, EditorMode::Split),
    ] {
        let (mut sync, _app, document_id) = ready_in(editor_id);
        assert_eq!(
            sync.submit(switch(mode), Some(document_id), 10),
            SubmitOutcome::NoOp,
            "{mode:?}"
        );
        assert!(
            sync.drain_actions().is_empty(),
            "{mode:?}: no claim to act on"
        );
        assert!(sync.queue.is_empty(), "{mode:?}: no queue entry");
        assert!(!sync.has_discards(), "{mode:?}: no discard");
    }
}

#[test]
fn preview_and_form_are_never_a_no_op() {
    for mode in [EditorMode::Preview, EditorMode::Form] {
        let (mut sync, _app, document_id) = ready_in(SourceEditorId::Text);
        assert!(
            matches!(
                sync.submit(switch(mode), Some(document_id), 10),
                SubmitOutcome::SnapshotRequested(_)
            ),
            "{mode:?} from Text"
        );
        // And again while it is in flight: accepted, not a no-op.
        assert_eq!(
            sync.submit(switch(mode), Some(document_id), 11),
            SubmitOutcome::Queued,
            "{mode:?} while {mode:?} is in flight"
        );
    }
}

#[test]
fn the_newest_switch_is_the_one_the_app_is_heading_for() {
    let (mut sync, _app, document_id) = ready_in(SourceEditorId::Text);
    sync.submit(switch(EditorMode::Preview), Some(document_id), 10);
    sync.queue.push_back(QueuedCommand {
        command: switch(EditorMode::Text),
        focus_token: None,
        deadline_ms: u64::MAX,
        scope: QueueScope::Anywhere,
    });

    // Preview is in flight, and Text is already queued after it -- so a
    // repeat Text click still reaches the queue (task 022: a queued switch is
    // never a no-op), coalescing with itself rather than being dropped.
    assert_eq!(
        sync.submit(switch(EditorMode::Text), Some(document_id), 11),
        SubmitOutcome::Queued
    );
    assert_eq!(queued_commands(&sync), vec![switch(EditorMode::Text)]);
    // Form is where it ends instead: it replaces the queued Text.
    assert_eq!(
        sync.submit(switch(EditorMode::Form), Some(document_id), 12),
        SubmitOutcome::Queued
    );
    assert_eq!(queued_commands(&sync), vec![switch(EditorMode::Form)]);
}
