use bekoedit_ui_contract::{BRIDGE_SCHEMA_VERSION, source_editor::SourceEditorEvent};

use super::{LifecycleEffect, LifecycleState, TransitionError, ready_reducer};

#[test]
fn ordinary_change_advances_only_the_matching_ready_stream() {
    let mut reducer = ready_reducer();
    let identity = reducer.ready_editor().unwrap().identity;
    reducer
        .accept_change(
            &SourceEditorEvent::Change {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                identity,
                seq: 1,
                text: "typed".into(),
                composing: false,
            },
            4,
        )
        .unwrap();
    assert!(matches!(
        reducer.state,
        LifecycleState::Ready(editor)
            if editor.identity == identity && editor.last_seq == 1 && editor.revision == 4
    ));
    assert_eq!(
        reducer.accept_change(
            &SourceEditorEvent::Change {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                identity,
                seq: 1,
                text: "stale".into(),
                composing: false,
            },
            5,
        ),
        Err(TransitionError::Stale)
    );
}

#[test]
fn explicit_unmount_invalidates_ready_before_destroy_acknowledgement() {
    let mut reducer = ready_reducer();
    let identity = reducer.ready_editor().unwrap().identity;
    let LifecycleEffect::Destroy(retired, operation_id) =
        reducer.begin_unmount(100).unwrap().unwrap()
    else {
        unreachable!()
    };
    assert_eq!(retired, identity);
    assert!(matches!(
        reducer.state,
        LifecycleState::Unmounting {
            retired: actual,
            operation,
            waiting: None,
        } if actual == identity && operation.operation_id == operation_id
    ));
}
