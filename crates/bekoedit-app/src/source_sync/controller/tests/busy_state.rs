//! Task 021: `busy_lifecycle_state` is exactly the states `submit` answers
//! `Busy` to, for a command that names the current document.

use std::collections::BTreeSet;

use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEpoch,
};

use super::*;
use crate::source_sync::lifecycle::{
    HeldEditor, HoldCertainty, PendingOperation, ReadyEditor, SessionFingerprint,
};

const DOCUMENT: u64 = 7;

fn identity() -> EditorIdentity {
    EditorIdentity {
        instance_id: EditorInstanceId::new(1),
        editor_id: SourceEditorId::Text,
        document_id: DOCUMENT,
        epoch: SourceEpoch::new(1),
    }
}

fn ready() -> ReadyEditor {
    ReadyEditor {
        identity: identity(),
        revision: 1,
        last_seq: 0,
    }
}

fn held() -> HeldEditor {
    HeldEditor {
        ready: ready(),
        snapshot_operation: OperationId::new(2),
        certainty: HoldCertainty::Confirmed,
    }
}

fn operation() -> PendingOperation {
    PendingOperation {
        operation_id: OperationId::new(3),
        deadline_ms: 1_000,
    }
}

fn intent() -> MountIntent {
    MountIntent {
        editor_id: SourceEditorId::Text,
        document_id: DOCUMENT,
        revision: 1,
    }
}

/// Exhaustive: adding a `LifecycleState` variant stops this compiling until
/// the truth table below is extended with it.
fn variant_name(state: &LifecycleState) -> &'static str {
    match state {
        LifecycleState::Unmounted => "Unmounted",
        LifecycleState::Mounting { .. } => "Mounting",
        LifecycleState::Initializing { .. } => "Initializing",
        LifecycleState::Ready(_) => "Ready",
        LifecycleState::SnapshotPending { .. } => "SnapshotPending",
        LifecycleState::BarrierHeld { .. } => "BarrierHeld",
        LifecycleState::ResumePending { .. } => "ResumePending",
        LifecycleState::RefreshPending { .. } => "RefreshPending",
        LifecycleState::Unmounting { .. } => "Unmounting",
        LifecycleState::Unavailable { .. } => "Unavailable",
    }
}

fn in_state(state: LifecycleState, waiting: bool) -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = state;
    if waiting {
        sync.waiting_command = Some(PendingCommand {
            command: SourceCommand::SaveNow,
            focus_token: None,
        });
    }
    sync
}

fn cases() -> Vec<(&'static str, SourceSyncState)> {
    let mounting = || LifecycleState::Mounting {
        intent: intent(),
        identity: identity(),
        relay: operation(),
        relay_ready: false,
        bundle_ready: false,
        takeover: None,
    };
    let initializing = || LifecycleState::Initializing {
        identity: identity(),
        revision: 1,
        operation: operation(),
    };
    let unmounting = |waiting| LifecycleState::Unmounting {
        retired: identity(),
        operation: operation(),
        waiting,
    };
    vec![
        ("unmounted", in_state(LifecycleState::Unmounted, false)),
        ("mounting, slot free", in_state(mounting(), false)),
        ("mounting, slot taken", in_state(mounting(), true)),
        ("initializing, slot free", in_state(initializing(), false)),
        ("initializing, slot taken", in_state(initializing(), true)),
        ("ready", in_state(LifecycleState::Ready(ready()), false)),
        (
            "snapshot pending",
            in_state(
                LifecycleState::SnapshotPending {
                    editor: ready(),
                    command: SourceCommand::SaveNow,
                    operation: operation(),
                },
                false,
            ),
        ),
        (
            "barrier held",
            in_state(
                LifecycleState::BarrierHeld {
                    editor: held(),
                    command: SourceCommand::SaveNow,
                    before: SessionFingerprint {
                        document_id: Some(DOCUMENT),
                        revision: Some(1),
                        source_token: DOCUMENT,
                    },
                },
                false,
            ),
        ),
        (
            "resume pending",
            in_state(
                LifecycleState::ResumePending {
                    editor: held(),
                    operation: operation(),
                },
                false,
            ),
        ),
        (
            "refresh pending",
            in_state(
                LifecycleState::RefreshPending {
                    editor: held(),
                    new_epoch: SourceEpoch::new(2),
                    revision: 2,
                    operation: operation(),
                },
                false,
            ),
        ),
        ("unmounting", in_state(unmounting(None), false)),
        (
            "unmounting, a mount waiting",
            in_state(unmounting(Some(intent())), false),
        ),
        (
            "unavailable, nothing retired",
            in_state(LifecycleState::Unavailable { retired: None }, false),
        ),
        (
            "unavailable, retired",
            in_state(
                LifecycleState::Unavailable {
                    retired: Some(identity()),
                },
                false,
            ),
        ),
    ]
}

#[test]
fn busy_lifecycle_state_is_exactly_what_submit_answers_busy_to() {
    for (label, mut sync) in cases() {
        let reported = sync.busy_lifecycle_state();
        // A command that is neither a same-mode no-op nor otherwise special.
        let outcome = sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);
        assert_eq!(
            reported.is_some(),
            outcome == SubmitOutcome::Busy,
            "{label}: busy_lifecycle_state() = {reported:?}, submit answered {outcome:?}"
        );
    }
}

#[test]
fn busy_lifecycle_state_names_the_state_it_reports() {
    for (label, sync) in cases() {
        if let Some(name) = sync.busy_lifecycle_state() {
            assert_eq!(
                name,
                variant_name(&sync.lifecycle.state),
                "{label}: the reported name is the state's own"
            );
        }
    }
}

#[test]
fn the_truth_table_covers_every_lifecycle_variant() {
    let covered: BTreeSet<_> = cases()
        .iter()
        .map(|(_, sync)| variant_name(&sync.lifecycle.state))
        .collect();
    assert_eq!(covered.len(), 10, "covered: {covered:?}");
}

#[test]
fn the_accessor_changes_nothing() {
    for (label, sync) in cases() {
        let before = format!("{:?}", sync.lifecycle.state);
        let waiting = sync.waiting_command.clone();
        let _ = sync.busy_lifecycle_state();
        assert_eq!(before, format!("{:?}", sync.lifecycle.state), "{label}");
        assert_eq!(waiting, sync.waiting_command, "{label}");
    }
}
