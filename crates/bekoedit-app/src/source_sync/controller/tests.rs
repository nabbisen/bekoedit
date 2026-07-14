use bekoedit_core::AppState;
use bekoedit_fs::RecoveryStore;
use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION, EditorMode,
    source_editor::{SourceEditorEvent, SourceEditorId},
};

use super::*;

fn app() -> AppState {
    let dir = tempfile::tempdir().unwrap().keep();
    let mut app = AppState::new(
        RecoveryStore::at(dir.join(".recovery")),
        dir.join(".recent.json"),
        100,
    );
    app.new_untitled();
    app
}

fn intent(app: &AppState) -> MountIntent {
    intent_for(app, SourceEditorId::Text)
}

fn intent_for(app: &AppState, editor_id: SourceEditorId) -> MountIntent {
    let session = app.session.as_ref().unwrap();
    MountIntent {
        editor_id,
        document_id: session.document_id,
        revision: session.revision,
    }
}

fn make_ready(sync: &mut SourceSyncState, app: &mut AppState) {
    make_ready_as(sync, app, SourceEditorId::Text);
}

fn make_ready_as(sync: &mut SourceSyncState, app: &mut AppState, editor_id: SourceEditorId) {
    sync.start_bundle_probe(0);
    let ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(probe)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    sync.handle_event(
        SourceEditorEvent::BundleReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: probe,
        },
        app,
        1,
    )
    .unwrap();
    assert_eq!(
        sync.mount(intent_for(app, editor_id), 2),
        MountOutcome::Started
    );
    let ControllerAction::Lifecycle(LifecycleEffect::InstallRelay(identity, relay_operation)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    sync.handle_event(
        SourceEditorEvent::RelayReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: relay_operation,
            identity,
        },
        app,
        3,
    )
    .unwrap();
    let ControllerAction::Lifecycle(LifecycleEffect::Init(_, init_operation, _)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    let revision = app.session.as_ref().unwrap().revision;
    sync.handle_event(
        SourceEditorEvent::EditorReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: init_operation,
            identity,
            revision,
            reused: false,
        },
        app,
        4,
    )
    .unwrap();
}

#[test]
fn selecting_the_current_text_or_split_mode_is_a_no_op() {
    for (editor_id, mode) in [
        (SourceEditorId::Text, EditorMode::Text),
        (SourceEditorId::Split, EditorMode::Split),
    ] {
        let mut app = app();
        let mut sync = SourceSyncState::default();
        make_ready_as(&mut sync, &mut app, editor_id);
        let ready = sync.lifecycle.ready_editor().unwrap();
        assert_eq!(
            sync.submit(
                SourceCommand::SwitchMode(mode),
                Some(ready.identity.document_id),
                10,
            ),
            SubmitOutcome::NoOp
        );
        assert_eq!(sync.lifecycle.ready_editor(), Some(ready));
        assert!(sync.drain_actions().is_empty());
    }
}

#[test]
fn immediate_preview_waits_for_ready_then_requests_snapshot() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    sync.start_bundle_probe(0);
    let ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(probe)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(sync.mount(intent(&app), 1), MountOutcome::Started);
    let ControllerAction::Lifecycle(LifecycleEffect::InstallRelay(identity, relay_operation)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        sync.submit(
            SourceCommand::SwitchMode(EditorMode::Preview),
            app.session.as_ref().map(|session| session.document_id),
            2,
        ),
        SubmitOutcome::WaitingForReady
    );
    assert_eq!(
        sync.submit(SourceCommand::SaveNow, Some(intent(&app).document_id), 3),
        SubmitOutcome::Busy
    );
    sync.handle_event(
        SourceEditorEvent::RelayReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: relay_operation,
            identity,
        },
        &mut app,
        4,
    )
    .unwrap();
    assert!(sync.drain_actions().is_empty());
    sync.handle_event(
        SourceEditorEvent::BundleReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: probe,
        },
        &mut app,
        5,
    )
    .unwrap();
    let ControllerAction::Lifecycle(LifecycleEffect::Init(_, init_operation, _)) =
        sync.drain_actions().pop().unwrap()
    else {
        unreachable!()
    };
    sync.handle_event(
        SourceEditorEvent::EditorReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: init_operation,
            identity,
            revision: app.session.as_ref().unwrap().revision,
            reused: false,
        },
        &mut app,
        6,
    )
    .unwrap();
    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Lifecycle(LifecycleEffect::RequestSnapshot(
            actual,
            _
        ))] if *actual == identity
    ));
}

#[test]
fn accepted_snapshot_updates_canonical_text_before_command_execution() {
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
    sync.handle_event(
        SourceEditorEvent::Snapshot {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            seq: 1,
            text: "typed before preview\n".into(),
            composing: false,
        },
        &mut app,
        11,
    )
    .unwrap();
    assert_eq!(
        app.session.as_ref().unwrap().canonical_text,
        "typed before preview\n"
    );
    let held_state = sync.lifecycle.state.clone();
    let current = app.session.as_ref().unwrap();
    assert_eq!(
        sync.mount(
            MountIntent {
                editor_id: SourceEditorId::Text,
                document_id: current.document_id,
                revision: current.revision,
            },
            12,
        ),
        MountOutcome::AlreadyCurrent
    );
    assert_eq!(sync.lifecycle.state, held_state);
    let actions = sync.drain_actions();
    assert_eq!(
        actions,
        vec![ControllerAction::Execute {
            command: SourceCommand::SwitchMode(EditorMode::Preview),
            protected: true,
        }]
    );
    let mut mode = EditorMode::Text;
    let mut settings_open = false;
    crate::source_sync::commands::execute(
        &mut app,
        &mut mode,
        &mut settings_open,
        &SourceCommand::SwitchMode(EditorMode::Preview),
        12,
    )
    .unwrap();
    assert_eq!(mode, EditorMode::Preview);
    assert_eq!(
        sync.command_completed(true, fingerprint(&app), 13).unwrap(),
        CommandDisposition::Destroy
    );
    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Lifecycle(LifecycleEffect::Destroy(
            actual,
            _
        ))] if *actual == identity
    ));
}

#[test]
fn duplicate_terminal_event_is_a_stale_no_op() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let ready = sync.lifecycle.ready_editor().unwrap();
    assert_eq!(
        sync.handle_event(
            SourceEditorEvent::EditorReady {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: bekoedit_ui_contract::source_editor::OperationId::new(99),
                identity: ready.identity,
                revision: ready.revision,
                reused: true,
            },
            &mut app,
            20,
        )
        .unwrap(),
        EventOutcome::Stale
    );
    assert!(sync.is_ready(SourceEditorId::Text, ready.identity.document_id));
}

#[test]
fn stale_snapshot_operation_cannot_publish_text_or_execute_command() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let original = app.session.as_ref().unwrap().canonical_text.clone();
    let document_id = app.session.as_ref().unwrap().document_id;
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let SubmitOutcome::SnapshotRequested(operation_id) =
        sync.submit(SourceCommand::SaveNow, Some(document_id), 10)
    else {
        unreachable!()
    };
    sync.drain_actions();
    let outcome = sync
        .handle_event(
            SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: bekoedit_ui_contract::source_editor::OperationId::new(
                    operation_id.get() + 1,
                ),
                identity,
                seq: 1,
                text: "stale payload".into(),
                composing: false,
            },
            &mut app,
            11,
        )
        .unwrap();
    assert_eq!(outcome, EventOutcome::Stale);
    assert_eq!(app.session.as_ref().unwrap().canonical_text, original);
    assert!(sync.drain_actions().is_empty());
}

#[test]
fn unsupported_change_cannot_mutate_canonical_or_lifecycle_state() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let ready = sync.lifecycle.ready_editor().unwrap();
    let before_text = app.session.as_ref().unwrap().canonical_text.clone();
    let before_revision = app.session.as_ref().unwrap().revision;
    let before_state = sync.lifecycle.state.clone();
    let result = sync.handle_event(
        SourceEditorEvent::Change {
            protocol_version: 1,
            identity: ready.identity,
            seq: 1,
            text: "unsupported change".into(),
            composing: false,
        },
        &mut app,
        30,
    );
    assert!(matches!(result, Err(SourceSyncError::UnsupportedVersion)));
    assert_eq!(app.session.as_ref().unwrap().canonical_text, before_text);
    assert_eq!(app.session.as_ref().unwrap().revision, before_revision);
    assert_eq!(sync.lifecycle.state, before_state);
    assert!(sync.drain_actions().is_empty());
}

#[test]
fn unsupported_snapshot_cannot_mutate_canonical_or_pending_operation() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let SubmitOutcome::SnapshotRequested(operation_id) =
        sync.submit(SourceCommand::SaveNow, Some(document_id), 30)
    else {
        unreachable!()
    };
    sync.drain_actions();
    let before_text = app.session.as_ref().unwrap().canonical_text.clone();
    let before_revision = app.session.as_ref().unwrap().revision;
    let before_state = sync.lifecycle.state.clone();
    let result = sync.handle_event(
        SourceEditorEvent::Snapshot {
            protocol_version: 1,
            operation_id,
            identity,
            seq: 1,
            text: "unsupported snapshot".into(),
            composing: false,
        },
        &mut app,
        31,
    );
    assert!(matches!(result, Err(SourceSyncError::UnsupportedVersion)));
    assert_eq!(app.session.as_ref().unwrap().canonical_text, before_text);
    assert_eq!(app.session.as_ref().unwrap().revision, before_revision);
    assert_eq!(sync.lifecycle.state, before_state);
    assert!(sync.drain_actions().is_empty());
}

mod instance_drop;
