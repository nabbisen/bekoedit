use super::*;

#[test]
fn accepted_waiting_mount_is_reported_as_queued() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let current = sync.lifecycle.ready_editor().unwrap().identity;
    let handle = sync
        .mount_handle(current.editor_id, current.document_id)
        .unwrap();
    sync.unmount(handle, 20);
    sync.drain_actions();
    app.new_untitled();
    assert_eq!(sync.mount(intent(&app), 21), MountOutcome::Queued);
}

#[test]
fn stale_component_drop_with_wrong_surface_cannot_destroy_current_editor() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let ready = sync.lifecycle.ready_editor().unwrap();
    sync.unmount(
        EditorMountHandle {
            instance_id: ready.identity.instance_id,
            editor_id: SourceEditorId::Split,
            document_id: ready.identity.document_id,
        },
        20,
    );
    assert!(sync.is_ready(SourceEditorId::Text, ready.identity.document_id));
    assert!(sync.drain_actions().is_empty());
}

#[test]
fn old_instance_drop_is_ignored_after_same_surface_replacement() {
    for replace_document in [false, true] {
        let mut app = app();
        let mut sync = SourceSyncState::default();
        make_ready(&mut sync, &mut app);
        let old_identity = sync.lifecycle.ready_editor().unwrap().identity;
        let old_handle = sync
            .mount_handle(old_identity.editor_id, old_identity.document_id)
            .unwrap();

        sync.force_unmount(20);
        let ControllerAction::Lifecycle(LifecycleEffect::Destroy(retired, destroy_operation)) =
            sync.drain_actions().pop().unwrap()
        else {
            unreachable!()
        };
        if replace_document {
            app.new_untitled();
        }
        assert_eq!(sync.mount(intent(&app), 21), MountOutcome::Queued);
        sync.handle_event(
            SourceEditorEvent::Destroyed {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: destroy_operation,
                identity: retired,
            },
            &mut app,
            22,
        )
        .unwrap();
        let ControllerAction::Lifecycle(LifecycleEffect::InstallRelay(
            replacement,
            relay_operation,
        )) = sync.drain_actions().pop().unwrap()
        else {
            unreachable!()
        };
        assert_ne!(replacement.instance_id, old_handle.instance_id);
        sync.handle_event(
            SourceEditorEvent::RelayReady {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: relay_operation,
                identity: replacement,
            },
            &mut app,
            23,
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
                identity: replacement,
                revision: app.session.as_ref().unwrap().revision,
                reused: false,
            },
            &mut app,
            24,
        )
        .unwrap();

        sync.unmount(old_handle, 25);
        assert!(sync.is_ready(SourceEditorId::Text, replacement.document_id));
        assert_eq!(
            sync.mount_handle(SourceEditorId::Text, replacement.document_id),
            Some(EditorMountHandle {
                instance_id: replacement.instance_id,
                editor_id: SourceEditorId::Text,
                document_id: replacement.document_id,
            })
        );
        assert!(sync.drain_actions().is_empty());
    }
}

#[test]
fn shutdown_clears_pending_command_and_returns_only_best_effort_destroy() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    assert!(matches!(
        sync.submit(SourceCommand::SaveNow, Some(document_id), 20),
        SubmitOutcome::SnapshotRequested(_)
    ));
    let effect = sync.shutdown(21).unwrap();
    assert!(matches!(effect, LifecycleEffect::Destroy(identity, _)
        if identity.document_id == document_id));
    assert!(sync.drain_actions().is_empty());
    assert!(matches!(
        sync.lifecycle.state,
        LifecycleState::Unmounting { waiting: None, .. }
    ));
}

#[test]
fn repeated_relay_loss_then_retry_reaches_typed_install_and_fresh_ready() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    sync.relay_generation_started(1);
    assert!(sync.relay_generation_ready(1, 10));
    let retired = sync.lifecycle.ready_editor().unwrap().identity;
    assert!(matches!(
        sync.submit(SourceCommand::SaveNow, Some(retired.document_id), 20),
        SubmitOutcome::SnapshotRequested(_)
    ));
    assert!(sync.has_actions());

    assert!(sync.relay_disconnected(1));
    assert!(sync.drain_actions().is_empty());
    assert_eq!(
        sync.lifecycle.state,
        LifecycleState::Unavailable {
            retired: Some(retired)
        }
    );
    assert_eq!(sync.mount(intent(&app), 21), MountOutcome::Started);
    assert!(!sync.has_dispatchable_actions());
    assert!(sync.drain_dispatchable_actions().is_empty());
    assert!(sync.has_actions());
    for generation in 2..=21 {
        sync.relay_generation_started(generation);
        assert!(!sync.relay_disconnected(generation));
        assert!(matches!(
            sync.lifecycle.state,
            LifecycleState::Unmounting {
                retired: current,
                waiting: Some(_),
                ..
            } if current == retired
        ));
        assert!(sync.drain_dispatchable_actions().is_empty());
    }
    assert_eq!(sync.tick(20_000).unwrap(), TickOutcome::Idle);
    assert!(sync.has_actions());

    sync.relay_generation_started(41);
    assert!(!sync.relay_generation_ready(1, 20_000));
    assert!(!sync.has_dispatchable_actions());
    assert!(sync.relay_generation_ready(41, 20_000));
    assert!(sync.has_dispatchable_actions());
    assert_eq!(sync.tick(20_999).unwrap(), TickOutcome::Idle);
    let actions = sync.drain_dispatchable_actions();
    assert_eq!(actions.len(), 1);
    let ControllerAction::Lifecycle(LifecycleEffect::Destroy(identity, destroy_operation)) =
        actions.into_iter().next().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(identity, retired);
    sync.handle_event(
        SourceEditorEvent::Destroyed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: destroy_operation,
            identity,
        },
        &mut app,
        21_000,
    )
    .unwrap();
    let ControllerAction::Lifecycle(LifecycleEffect::InstallRelay(replacement, relay_operation)) =
        sync.drain_dispatchable_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_ne!(replacement.instance_id, retired.instance_id);
    sync.handle_event(
        SourceEditorEvent::RelayReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: relay_operation,
            identity: replacement,
        },
        &mut app,
        21_001,
    )
    .unwrap();
    let ControllerAction::Lifecycle(LifecycleEffect::Init(initialized, init_operation, _)) =
        sync.drain_dispatchable_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(initialized, replacement);
    let revision = app.session.as_ref().unwrap().revision;
    sync.handle_event(
        SourceEditorEvent::EditorReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: init_operation,
            identity: replacement,
            revision,
            reused: false,
        },
        &mut app,
        21_002,
    )
    .unwrap();
    assert!(sync.is_ready(SourceEditorId::Text, replacement.document_id));
}

#[test]
fn unacknowledged_attempt_preserves_retry_state_and_single_action() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    sync.relay_generation_started(1);
    assert!(sync.relay_generation_ready(1, 10));
    let retired = sync.lifecycle.ready_editor().unwrap().identity;
    assert!(sync.relay_disconnected(1));

    assert_eq!(sync.mount(intent(&app), 20), MountOutcome::Started);
    let retry_state = sync.lifecycle.state.clone();
    sync.relay_generation_started(2);
    assert!(!sync.relay_disconnected(2));
    assert_eq!(sync.lifecycle.state, retry_state);
    assert_eq!(sync.tick(10_000).unwrap(), TickOutcome::Idle);
    assert!(sync.drain_dispatchable_actions().is_empty());

    sync.relay_generation_started(3);
    assert!(sync.relay_generation_ready(3, 10_000));
    let actions = sync.drain_dispatchable_actions();
    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions.as_slice(),
        [ControllerAction::Lifecycle(LifecycleEffect::Destroy(identity, _))]
            if *identity == retired
    ));
}

#[test]
fn acknowledged_loss_reissues_an_in_flight_bundle_probe_on_recovery() {
    let mut sync = SourceSyncState::default();
    sync.relay_generation_started(1);
    assert!(sync.relay_generation_ready(1, 0));
    sync.start_bundle_probe(0);
    let ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(first)) =
        sync.drain_dispatchable_actions().pop().unwrap()
    else {
        unreachable!()
    };

    assert!(!sync.relay_disconnected(1));
    sync.relay_generation_started(2);
    assert!(sync.relay_generation_ready(2, 100));
    sync.start_bundle_probe(100);
    let ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(second)) =
        sync.drain_dispatchable_actions().pop().unwrap()
    else {
        unreachable!()
    };
    assert_ne!(first, second);
}
