//! Task 022: the focus layer's no-op decision follows the controller's
//! effective target, not the UI mode signal alone.

use super::*;

fn ready_editor(editor_id: SourceEditorId) -> crate::source_sync::lifecycle::ReadyEditor {
    use bekoedit_ui_contract::source_editor::{EditorIdentity, EditorInstanceId, SourceEpoch};
    crate::source_sync::lifecycle::ReadyEditor {
        identity: EditorIdentity {
            instance_id: EditorInstanceId::new(1),
            editor_id,
            document_id: 1,
            epoch: SourceEpoch::new(1),
        },
        revision: 1,
        last_seq: 0,
    }
}

fn in_flight_to(mode: EditorMode) -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = crate::source_sync::lifecycle::LifecycleState::SnapshotPending {
        editor: ready_editor(SourceEditorId::Text),
        command: SourceCommand::SwitchMode(mode),
        operation: crate::source_sync::lifecycle::PendingOperation {
            operation_id: bekoedit_ui_contract::source_editor::OperationId::new(9),
            deadline_ms: u64::MAX,
        },
    };
    sync
}

#[test]
fn case_a_a_click_that_disagrees_with_an_in_flight_switch_claims_focus() {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state =
        crate::source_sync::lifecycle::LifecycleState::Ready(ready_editor(SourceEditorId::Text));

    // The real no-op: Text mounted, nothing pending.
    assert!(!claims_focus(
        &SourceCommand::SwitchMode(EditorMode::Text),
        EditorMode::Text,
        &sync
    ));

    // Case A: Preview is in flight. The UI mode argument still says Text
    // -- it does not change until the switch executes -- but the click
    // must still claim focus, because the app is no longer heading there.
    sync = in_flight_to(EditorMode::Preview);
    assert!(
        claims_focus(
            &SourceCommand::SwitchMode(EditorMode::Text),
            EditorMode::Text,
            &sync
        ),
        "the app is heading to Preview: this Text click is not a no-op"
    );
}

#[test]
fn a_switch_to_the_in_flight_targets_own_mode_still_claims_nothing() {
    // Not case A: the in-flight switch already claimed the editor: a
    // repeat click on the same target cannot do anything more, and the
    // original claim (already in flight) is the one that will take
    // focus once the editor is ready.
    let sync = in_flight_to(EditorMode::Text);
    assert!(!claims_focus(
        &SourceCommand::SwitchMode(EditorMode::Text),
        EditorMode::Text,
        &sync
    ));
}

#[test]
fn the_uis_mode_signal_is_not_consulted_for_a_switchs_no_op_decision() {
    // "One notion of the target" (task 022 §4): varying the UI mode
    // argument alone must never change a `SwitchMode`'s no-op decision --
    // only `sync` may. `OpenDocument`'s claim legitimately depends on the
    // UI mode (task 014 §2) and is untouched; this is about `SwitchMode`
    // only, which is where slice 1 left a second, disagreeing copy.
    let mounted_text = {
        let mut sync = SourceSyncState::default();
        sync.lifecycle.state = crate::source_sync::lifecycle::LifecycleState::Ready(ready_editor(
            SourceEditorId::Text,
        ));
        sync
    };
    for ui_mode in ALL_MODES {
        assert!(
            !claims_focus(
                &SourceCommand::SwitchMode(EditorMode::Text),
                ui_mode,
                &mounted_text
            ),
            "mounted Text, nothing pending: {ui_mode:?}"
        );
    }

    let preview_in_flight = in_flight_to(EditorMode::Preview);
    for ui_mode in ALL_MODES {
        assert!(
            claims_focus(
                &SourceCommand::SwitchMode(EditorMode::Text),
                ui_mode,
                &preview_in_flight
            ),
            "Preview in flight, regardless of the UI mode signal: {ui_mode:?}"
        );
    }
}

#[test]
fn commands_without_an_editor_destination_claim_nothing() {
    for current in ALL_MODES {
        for command in [
            SourceCommand::OpenSettings,
            SourceCommand::SaveNow,
            SourceCommand::CloseWorkspace,
        ] {
            assert_eq!(focus_target(&command, current), None, "{command:?}");
        }
    }
}
