use bekoedit_ui_contract::source_editor::{OperationId, SourceEditorId, SourceEpoch};

use super::{HoldCertainty, LifecycleEffect, LifecycleReducer, LifecycleState, RESUME_DEADLINE_MS};

fn ready_reducer() -> LifecycleReducer {
    let mut reducer = LifecycleReducer::default();
    reducer.begin_mount(SourceEditorId::Text, 7, 3, 10);
    assert!(reducer.mark_bundle_ready().is_none());
    let instance = match reducer.state {
        LifecycleState::Mounting { identity, .. } => identity.instance_id,
        ref other => panic!("expected mounting, got {other:?}"),
    };
    let init = reducer.mark_relay_ready(instance).unwrap();
    let (identity, operation) = match init {
        LifecycleEffect::Init(identity, operation) => (identity, operation),
        other => panic!("expected init, got {other:?}"),
    };
    assert!(reducer.editor_ready(identity, operation));
    reducer
}

#[test]
fn mount_is_not_ready_until_both_prerequisites_and_matching_ready() {
    let mut reducer = LifecycleReducer::default();
    reducer.begin_mount(SourceEditorId::Text, 7, 3, 10);
    assert!(!matches!(reducer.state, LifecycleState::Ready(_)));
    assert!(reducer.mark_bundle_ready().is_none());
    let instance = match reducer.state {
        LifecycleState::Mounting { identity, .. } => identity.instance_id,
        _ => unreachable!(),
    };
    let init = reducer.mark_relay_ready(instance).unwrap();
    let LifecycleEffect::Init(identity, operation) = init else {
        unreachable!()
    };
    assert!(!reducer.editor_ready(identity, OperationId(operation.0 + 1)));
    assert!(reducer.editor_ready(identity, operation));
}

#[test]
fn snapshot_hold_requires_acknowledged_resume_before_ready() {
    let mut reducer = ready_reducer();
    let request = reducer.begin_snapshot(100).unwrap();
    let LifecycleEffect::RequestSnapshot(_, snapshot_operation) = request else {
        unreachable!()
    };
    assert!(reducer.snapshot_received(snapshot_operation, 4));
    assert!(matches!(reducer.state, LifecycleState::BarrierHeld(_)));
    let resume = reducer.begin_resume(110).unwrap();
    let LifecycleEffect::Resume(_, held_operation, resume_operation) = resume else {
        unreachable!()
    };
    assert_eq!(held_operation, snapshot_operation);
    assert!(!matches!(reducer.state, LifecycleState::Ready(_)));
    assert!(reducer.editing_resumed(resume_operation, held_operation));
    assert!(matches!(reducer.state, LifecycleState::Ready(_)));
}

#[test]
fn lost_snapshot_response_uses_possible_hold_and_resume_deadline() {
    let mut reducer = ready_reducer();
    reducer.begin_snapshot(100).unwrap();
    let resume = reducer.begin_resume(1000).unwrap();
    let LifecycleEffect::Resume(_, _, resume_operation) = resume else {
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
    assert!(!reducer.expire(resume_operation, 1000 + RESUME_DEADLINE_MS - 1));
    assert!(reducer.expire(resume_operation, 1000 + RESUME_DEADLINE_MS));
    assert_eq!(reducer.state, LifecycleState::Unavailable);
}

#[test]
fn refresh_rolls_epoch_only_after_matching_acknowledgement() {
    let mut reducer = ready_reducer();
    let old_epoch = match reducer.state {
        LifecycleState::Ready(editor) => editor.identity.epoch,
        _ => unreachable!(),
    };
    let LifecycleEffect::RequestSnapshot(_, snapshot_operation) =
        reducer.begin_snapshot(100).unwrap()
    else {
        unreachable!()
    };
    reducer.snapshot_received(snapshot_operation, 1);
    let LifecycleEffect::ApplyDocument(_, new_epoch, refresh_operation) =
        reducer.begin_refresh(4, 120).unwrap()
    else {
        unreachable!()
    };
    assert_ne!(old_epoch, new_epoch);
    assert!(!reducer.document_applied(refresh_operation, SourceEpoch(new_epoch.0 + 1), 4));
    assert!(reducer.document_applied(refresh_operation, new_epoch, 4));
    assert!(matches!(
        reducer.state,
        LifecycleState::Ready(editor) if editor.identity.epoch == new_epoch && editor.revision == 4
    ));
}

#[test]
fn destroy_requires_matching_instance_and_operation() {
    let mut reducer = ready_reducer();
    let identity = match reducer.state {
        LifecycleState::Ready(editor) => editor.identity,
        _ => unreachable!(),
    };
    let LifecycleEffect::Destroy(_, operation) = reducer.begin_destroy(200).unwrap() else {
        unreachable!()
    };
    assert!(!reducer.destroyed(identity.instance_id, OperationId(operation.0 + 1)));
    assert!(reducer.destroyed(identity.instance_id, operation));
    assert_eq!(reducer.state, LifecycleState::Unmounted);
}
