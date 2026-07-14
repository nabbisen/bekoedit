use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{OperationId, SourceEditorEvent, SourceEditorId},
};

use crate::source_sync::SourceCommand;

use super::{
    CommandDisposition, HoldCertainty, LifecycleEffect, LifecycleReducer, LifecycleState,
    MountIntent, RESUME_DEADLINE_MS, SNAPSHOT_DEADLINE_MS, SessionFingerprint, TransitionError,
};

fn intent(document_id: u64) -> MountIntent {
    MountIntent {
        editor_id: SourceEditorId::Text,
        document_id,
        revision: 3,
    }
}

fn ready_reducer() -> LifecycleReducer {
    let mut reducer = LifecycleReducer::default();
    let probe = reducer.begin_bundle_probe(0);
    reducer
        .handle_bundle_event(&SourceEditorEvent::BundleReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: probe,
        })
        .unwrap();
    let LifecycleEffect::InstallRelay(identity, relay_operation) =
        reducer.begin_mount(intent(7), 10).unwrap()
    else {
        unreachable!()
    };
    let init = reducer
        .handle_relay_event(
            &SourceEditorEvent::RelayReady {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: relay_operation,
                identity,
            },
            20,
        )
        .unwrap()
        .unwrap();
    let LifecycleEffect::Init(identity, init_operation, _) = init else {
        unreachable!()
    };
    assert_ne!(relay_operation, init_operation);
    reducer
        .handle_init_event(&SourceEditorEvent::EditorReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: init_operation,
            identity,
            revision: 3,
            reused: false,
        })
        .unwrap();
    reducer
}

mod controller_support;

fn fingerprint(revision: u64, token: u64) -> SessionFingerprint {
    SessionFingerprint {
        document_id: Some(7),
        revision: Some(revision),
        source_token: token,
    }
}

fn held_reducer(command: SourceCommand, accepted_revision: u64) -> LifecycleReducer {
    let mut reducer = ready_reducer();
    let LifecycleEffect::RequestSnapshot(identity, operation_id) =
        reducer.begin_snapshot(command, 100).unwrap()
    else {
        unreachable!()
    };
    reducer
        .accept_snapshot(
            &SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                seq: 1,
                text: "typed".into(),
                composing: false,
            },
            accepted_revision,
            fingerprint(accepted_revision, 2),
        )
        .unwrap();
    reducer
}

#[test]
fn init_validates_operation_identity_and_revision() {
    let mut reducer = LifecycleReducer::default();
    let probe = reducer.begin_bundle_probe(0);
    reducer
        .handle_bundle_event(&SourceEditorEvent::BundleReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: probe,
        })
        .unwrap();
    let LifecycleEffect::InstallRelay(identity, relay_operation) =
        reducer.begin_mount(intent(7), 10).unwrap()
    else {
        unreachable!()
    };
    let LifecycleEffect::Init(_, init_operation, _) = reducer
        .handle_relay_event(
            &SourceEditorEvent::RelayReady {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: relay_operation,
                identity,
            },
            20,
        )
        .unwrap()
        .unwrap()
    else {
        unreachable!()
    };
    assert_ne!(relay_operation, init_operation);
    assert_eq!(
        reducer.handle_relay_event(
            &SourceEditorEvent::RelayReady {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: relay_operation,
                identity,
            },
            21,
        ),
        Err(TransitionError::InvalidState)
    );
    assert_eq!(
        reducer.handle_init_event(&SourceEditorEvent::EditorReady {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id: init_operation,
            identity,
            revision: 4,
            reused: false,
        }),
        Err(TransitionError::Stale)
    );
}

#[test]
fn accepted_snapshot_updates_revision_before_resume() {
    let mut reducer = held_reducer(SourceCommand::SaveNow, 4);
    let (disposition, effect) = reducer
        .command_completed(true, fingerprint(4, 2), 120)
        .unwrap();
    assert_eq!(disposition, CommandDisposition::Resume);
    let LifecycleEffect::Resume(identity, snapshot_operation_id, operation_id) = effect.unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        reducer.handle_resume_event(&SourceEditorEvent::EditingResumed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            snapshot_operation_id,
            revision: 3,
            was_held: true,
        }),
        Err(TransitionError::Stale)
    );
    reducer
        .handle_resume_event(&SourceEditorEvent::EditingResumed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            snapshot_operation_id,
            revision: 4,
            was_held: true,
        })
        .unwrap();
    assert!(matches!(reducer.state, LifecycleState::Ready(editor) if editor.revision == 4));
}

#[test]
fn snapshot_timeout_creates_possible_hold_then_resume_timeout_is_unavailable() {
    let mut reducer = ready_reducer();
    let LifecycleEffect::RequestSnapshot(_, snapshot_operation) =
        reducer.begin_snapshot(SourceCommand::SaveNow, 100).unwrap()
    else {
        unreachable!()
    };
    let LifecycleEffect::Resume(_, _, resume_operation) = reducer
        .timeout(snapshot_operation, 100 + SNAPSHOT_DEADLINE_MS)
        .unwrap()
        .unwrap()
    else {
        unreachable!()
    };
    assert!(matches!(
        reducer.state,
        LifecycleState::ResumePending {
            editor: super::HeldEditor {
                certainty: HoldCertainty::Possible,
                ..
            },
            ..
        }
    ));
    reducer
        .timeout(
            resume_operation,
            100 + SNAPSHOT_DEADLINE_MS + RESUME_DEADLINE_MS,
        )
        .unwrap();
    assert!(matches!(
        reducer.state,
        LifecycleState::Unavailable { retired: Some(_) }
    ));
}

#[test]
fn command_disposition_is_total_for_save_mutation_and_replacement() {
    let mut save = held_reducer(SourceCommand::SaveNow, 3);
    assert_eq!(
        save.command_completed(false, fingerprint(3, 2), 120)
            .unwrap()
            .0,
        CommandDisposition::Resume
    );

    let mut mutation = held_reducer(SourceCommand::MoveSectionUp(0), 3);
    assert_eq!(
        mutation
            .command_completed(true, fingerprint(4, 2), 120)
            .unwrap()
            .0,
        CommandDisposition::Refresh { revision: 4 }
    );

    let mut replacement = held_reducer(
        SourceCommand::OpenDocument(std::path::PathBuf::from("other.md")),
        3,
    );
    assert_eq!(
        replacement
            .command_completed(true, fingerprint(3, 1), 120)
            .unwrap()
            .0,
        CommandDisposition::Destroy
    );

    let mut settings = held_reducer(SourceCommand::OpenSettings, 3);
    assert_eq!(
        settings
            .command_completed(true, fingerprint(3, 2), 120)
            .unwrap()
            .0,
        CommandDisposition::Destroy
    );
}

#[test]
fn unavailable_retry_destroys_retired_before_waiting_mount() {
    let mut reducer = held_reducer(SourceCommand::SaveNow, 3);
    reducer.state = match reducer.state {
        LifecycleState::BarrierHeld { editor, .. } => LifecycleState::Unavailable {
            retired: Some(editor.ready.identity),
        },
        _ => unreachable!(),
    };
    let effect = reducer.begin_mount(intent(8), 200).unwrap();
    assert!(matches!(effect, LifecycleEffect::Destroy(_, _)));
    assert!(matches!(
        reducer.state,
        LifecycleState::Unmounting {
            waiting: Some(_),
            ..
        }
    ));
}

#[test]
fn unsupported_version_and_stale_terminal_are_rejected() {
    let mut reducer = ready_reducer();
    assert_eq!(
        reducer.handle_resume_event(&SourceEditorEvent::Trace {
            protocol_version: 1,
            instance_id: None,
            event: "old".into(),
        }),
        Err(TransitionError::UnsupportedVersion)
    );
    assert_eq!(
        reducer.timeout(OperationId::new(999), u64::MAX),
        Err(TransitionError::Stale)
    );
}

#[test]
fn composition_blocked_never_holds_and_rejected_snapshot_resumes_confirmed_hold() {
    use bekoedit_ui_contract::source_editor::BridgeFailureReason;

    let mut blocked = ready_reducer();
    let LifecycleEffect::RequestSnapshot(identity, operation_id) =
        blocked.begin_snapshot(SourceCommand::SaveNow, 100).unwrap()
    else {
        unreachable!()
    };
    blocked
        .handle_snapshot_blocked(&SourceEditorEvent::SnapshotBlocked {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity,
            reason: BridgeFailureReason::CompositionActive,
        })
        .unwrap();
    assert!(matches!(blocked.state, LifecycleState::Ready(_)));

    let mut rejected = ready_reducer();
    let LifecycleEffect::RequestSnapshot(identity, operation_id) = rejected
        .begin_snapshot(SourceCommand::SaveNow, 100)
        .unwrap()
    else {
        unreachable!()
    };
    let effect = rejected
        .reject_snapshot(
            &SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                seq: 1,
                text: "typed".into(),
                composing: false,
            },
            true,
            110,
        )
        .unwrap();
    assert!(matches!(effect, Some(LifecycleEffect::Resume(_, _, _))));
    assert!(matches!(
        rejected.state,
        LifecycleState::ResumePending {
            editor: super::HeldEditor {
                certainty: HoldCertainty::Confirmed,
                ..
            },
            ..
        }
    ));
}

#[test]
fn protected_snapshot_accepts_equal_seq_but_rejects_regression_and_revision_mismatch() {
    let mut equal = ready_reducer();
    let LifecycleEffect::RequestSnapshot(identity, operation_id) =
        equal.begin_snapshot(SourceCommand::SaveNow, 100).unwrap()
    else {
        unreachable!()
    };
    equal
        .accept_snapshot(
            &SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                seq: 0,
                text: String::new(),
                composing: false,
            },
            3,
            fingerprint(3, 2),
        )
        .unwrap();
    assert!(matches!(equal.state, LifecycleState::BarrierHeld { .. }));

    let mut lower = ready_reducer();
    if let LifecycleState::Ready(editor) = &mut lower.state {
        editor.last_seq = 2;
    }
    let LifecycleEffect::RequestSnapshot(identity, operation_id) =
        lower.begin_snapshot(SourceCommand::SaveNow, 100).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        lower.accept_snapshot(
            &SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                seq: 1,
                text: String::new(),
                composing: false,
            },
            3,
            fingerprint(3, 2),
        ),
        Err(TransitionError::Stale)
    );

    let mut mismatch = ready_reducer();
    let LifecycleEffect::RequestSnapshot(identity, operation_id) = mismatch
        .begin_snapshot(SourceCommand::SaveNow, 100)
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        mismatch.accept_snapshot(
            &SourceEditorEvent::Snapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                seq: 0,
                text: String::new(),
                composing: false,
            },
            4,
            fingerprint(3, 2),
        ),
        Err(TransitionError::Stale)
    );
}

#[test]
fn history_refresh_accepts_no_op_revision_and_fails_closed_for_invalid_identity() {
    let mut changed_token = held_reducer(
        SourceCommand::RestoreHistory(bekoedit_fs::HistoryEntry {
            original_path: std::path::PathBuf::from("doc.md"),
            text: "old".into(),
            saved_at_secs: 1,
            revision: 1,
        }),
        3,
    );
    assert_eq!(
        changed_token
            .command_completed(true, fingerprint(4, 99), 120)
            .unwrap()
            .0,
        CommandDisposition::Unavailable
    );

    let mut missing_revision = held_reducer(SourceCommand::MoveSectionDown(0), 3);
    assert_eq!(
        missing_revision
            .command_completed(
                true,
                SessionFingerprint {
                    document_id: Some(7),
                    revision: None,
                    source_token: 2,
                },
                120,
            )
            .unwrap()
            .0,
        CommandDisposition::Unavailable
    );

    let mut no_op = held_reducer(SourceCommand::MoveSectionUp(0), 3);
    let (disposition, effect) = no_op
        .command_completed(true, fingerprint(3, 2), 120)
        .unwrap();
    assert_eq!(disposition, CommandDisposition::Refresh { revision: 3 });
    let LifecycleEffect::ApplyDocument(identity, new_epoch, operation_id) = effect.unwrap() else {
        unreachable!()
    };
    assert_ne!(new_epoch, identity.epoch);
    let refreshed_identity = bekoedit_ui_contract::source_editor::EditorIdentity {
        epoch: new_epoch,
        ..identity
    };
    no_op
        .handle_document_event(&SourceEditorEvent::DocumentApplied {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            identity: refreshed_identity,
            revision: 3,
        })
        .unwrap();
    assert!(matches!(
        no_op.state,
        LifecycleState::Ready(editor)
            if editor.identity == refreshed_identity && editor.revision == 3
    ));

    let mut regressed = held_reducer(SourceCommand::MoveSectionDown(0), 3);
    assert_eq!(
        regressed
            .command_completed(true, fingerprint(2, 2), 120)
            .unwrap()
            .0,
        CommandDisposition::Unavailable
    );
}
