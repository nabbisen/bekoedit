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

#[test]
fn new_untitled_resets_the_editing_mode_to_text() {
    let mut app = app();
    let mut mode = EditorMode::Form;
    let mut settings_open = false;

    crate::source_sync::commands::execute(
        &mut app,
        &mut mode,
        &mut settings_open,
        &SourceCommand::NewUntitled,
        1,
    )
    .unwrap();

    assert_eq!(mode, EditorMode::Text);
    assert!(app.session.as_ref().unwrap().is_untitled);
    assert!(!settings_open);
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
fn focus_interaction_claim_is_exactly_once_and_supersession_is_monotonic() {
    let mut sync = SourceSyncState::default();
    let (first, superseded) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "first".into())
        .unwrap();
    assert_eq!(superseded, None);
    let (second, superseded) = sync
        .allocate_focus_interaction(SourceEditorId::Split, "second".into())
        .unwrap();
    assert_eq!(superseded, Some(first));
    assert!(second > first);
    assert_eq!(
        sync.claim_focus_interaction(first, FocusResolution::Armed),
        FocusClaim::Stale
    );
    assert_eq!(
        sync.claim_focus_interaction(second, FocusResolution::Armed),
        FocusClaim::Claimed
    );
    assert_eq!(
        sync.claim_focus_interaction(second, FocusResolution::ProceedWithoutFocus),
        FocusClaim::Stale
    );
}

#[test]
fn ready_focus_waits_for_command_execution_and_consumes_exactly_once() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    let (token, _) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "new-text".into())
        .unwrap();
    assert_eq!(
        sync.claim_focus_interaction(token, FocusResolution::Armed),
        FocusClaim::Claimed
    );
    make_ready(&mut sync, &mut app);
    assert!(sync.drain_actions().is_empty(), "old Ready cannot focus");

    let document_id = app.session.as_ref().map(|session| session.document_id);
    let _ = sync.focus_command_completed(Some(token), true, document_id);
    sync.force_unmount(10);
    sync.drain_actions();
    let session = app.session.as_ref().unwrap();
    let intent = MountIntent {
        editor_id: SourceEditorId::Text,
        document_id: session.document_id,
        revision: session.revision,
    };
    // A full remount is covered by lifecycle tests; exercise the exact Ready
    // action here with a fresh controller to keep this test focused.
    let mut remount = SourceSyncState {
        next_focus_token: token,
        pending_focus: sync.pending_focus.take(),
        ..SourceSyncState::default()
    };
    make_ready(&mut remount, &mut app);
    let actions = remount.drain_actions();
    assert!(matches!(
        actions.as_slice(),
        [ControllerAction::Focus {
            token: actual,
            identity,
            fingerprint,
        }] if *actual == token
            && identity.editor_id == intent.editor_id
            && fingerprint == "new-text"
    ));
    assert!(remount.drain_actions().is_empty());
}

#[test]
fn proceed_without_focus_clears_eligibility() {
    let mut sync = SourceSyncState::default();
    let (token, _) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "fallback".into())
        .unwrap();
    assert_eq!(
        sync.claim_focus_interaction(token, FocusResolution::ProceedWithoutFocus),
        FocusClaim::Claimed
    );
    let _ = sync.focus_command_completed(Some(token), true, Some(1));
    assert_eq!(sync.cancel_focus_interactions(), None);
}

#[test]
fn unprotected_execute_completion_is_correlated_to_its_exact_focus_token() {
    let mut app = app();
    let document_id = app.session.as_ref().unwrap().document_id;
    let mut sync = SourceSyncState::default();

    let (token_a, _) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "new-a".into())
        .unwrap();
    assert_eq!(
        sync.claim_focus_interaction(token_a, FocusResolution::Armed),
        FocusClaim::Claimed
    );
    assert_eq!(
        sync.submit_with_focus(
            SourceCommand::NewUntitled,
            Some(document_id),
            1,
            Some(token_a)
        ),
        SubmitOutcome::ExecuteQueued
    );

    let (token_b, superseded) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "new-b".into())
        .unwrap();
    assert_eq!(superseded, Some(token_a));
    assert_eq!(
        sync.claim_focus_interaction(token_b, FocusResolution::Armed),
        FocusClaim::Claimed
    );
    assert_eq!(
        sync.submit_with_focus(
            SourceCommand::NewUntitled,
            Some(document_id),
            2,
            Some(token_b)
        ),
        SubmitOutcome::ExecuteQueued
    );
    let actions = sync.drain_actions();
    assert!(matches!(
        actions.as_slice(),
        [
            ControllerAction::Execute {
                focus_token: Some(first),
                protected: false,
                ..
            },
            ControllerAction::Execute {
                focus_token: Some(second),
                protected: false,
                ..
            }
        ] if *first == token_a && *second == token_b
    ));

    assert_eq!(
        sync.focus_command_completed(Some(token_a), true, Some(document_id + 10)),
        None
    );
    assert!(sync.pending_focus.as_ref().is_some_and(|focus| {
        focus.token == token_b && !focus.command_executed && focus.result_document_id.is_none()
    }));
    assert_eq!(
        sync.focus_command_completed(Some(token_b), true, Some(document_id)),
        None
    );

    let wrong_identity = bekoedit_ui_contract::source_editor::EditorIdentity {
        instance_id: bekoedit_ui_contract::source_editor::EditorInstanceId::new(99),
        editor_id: SourceEditorId::Text,
        document_id: document_id + 1,
        epoch: bekoedit_ui_contract::source_editor::SourceEpoch::new(99),
    };
    sync.queue_ready_focus(wrong_identity);
    assert!(sync.drain_actions().is_empty());
    make_ready(&mut sync, &mut app);
    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Focus { token, identity, .. }]
            if *token == token_b && identity.document_id == document_id
    ));

    let mut failures = SourceSyncState::default();
    let (old, _) = failures
        .allocate_focus_interaction(SourceEditorId::Text, "old".into())
        .unwrap();
    failures.claim_focus_interaction(old, FocusResolution::Armed);
    let (current, _) = failures
        .allocate_focus_interaction(SourceEditorId::Text, "current".into())
        .unwrap();
    failures.claim_focus_interaction(current, FocusResolution::Armed);
    assert_eq!(
        failures.focus_command_completed(Some(old), false, None),
        None
    );
    assert_eq!(
        failures.pending_focus.as_ref().map(|focus| focus.token),
        Some(current)
    );
    assert_eq!(
        failures.focus_command_completed(Some(current), false, None),
        Some(current)
    );
    assert!(failures.pending_focus.is_none());
}

#[test]
fn protected_execute_carries_token_and_stale_completion_cannot_mutate_successor() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    let identity = sync.lifecycle.ready_editor().unwrap().identity;
    let (token_a, _) = sync
        .allocate_focus_interaction(SourceEditorId::Split, "split-a".into())
        .unwrap();
    sync.claim_focus_interaction(token_a, FocusResolution::Armed);
    let SubmitOutcome::SnapshotRequested(operation_id) = sync.submit_with_focus(
        SourceCommand::SwitchMode(EditorMode::Split),
        Some(document_id),
        10,
        Some(token_a),
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
            text: app.session.as_ref().unwrap().canonical_text.clone(),
            composing: false,
        },
        &mut app,
        11,
    )
    .unwrap();
    assert!(matches!(
        sync.drain_actions().as_slice(),
        [ControllerAction::Execute {
            focus_token: Some(token),
            protected: true,
            ..
        }] if *token == token_a
    ));

    let (token_b, _) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "text-b".into())
        .unwrap();
    sync.claim_focus_interaction(token_b, FocusResolution::Armed);
    assert_eq!(
        sync.focus_command_completed(Some(token_a), true, Some(document_id)),
        None
    );
    assert!(
        sync.pending_focus
            .as_ref()
            .is_some_and(|focus| { focus.token == token_b && !focus.command_executed })
    );
    assert_eq!(
        sync.focus_command_completed(Some(token_a), false, None),
        None
    );
    assert_eq!(
        sync.pending_focus.as_ref().map(|focus| focus.token),
        Some(token_b)
    );
    assert_eq!(
        sync.command_completed(false, fingerprint(&app), 12)
            .unwrap(),
        CommandDisposition::Resume
    );
    assert_eq!(
        sync.pending_focus.as_ref().map(|focus| focus.token),
        Some(token_b),
        "protected A failure cannot cancel pending B"
    );
}

#[test]
fn focus_token_exhaustion_never_reuses_the_javascript_maximum() {
    let mut sync = SourceSyncState {
        next_focus_token: interaction::MAX_JAVASCRIPT_FOCUS_TOKEN,
        ..SourceSyncState::default()
    };
    assert_eq!(
        sync.allocate_focus_interaction(SourceEditorId::Text, "exhausted".into()),
        None
    );
    assert_eq!(
        sync.allocate_focus_interaction(SourceEditorId::Text, "still-exhausted".into()),
        None
    );
}

mod protocol;

mod instance_drop;
