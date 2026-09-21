//! `busy_lifecycle_state` and `submit`, for a command that names the current
//! document, in every lifecycle state (task 021, extended by RFC-047 slice 1).
//! Nothing is answered `Busy` and dropped any more: a state the accessor
//! reports holds the command in the queue.

use std::collections::BTreeSet;

use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEpoch,
};

use super::types::{QueueScope, QueuedCommand};
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

/// `waiting`: a command is already queued, as `Mounting` and `Initializing`
/// report themselves only then.
fn in_state(state: LifecycleState, waiting: bool) -> SourceSyncState {
    let mut sync = SourceSyncState::default();
    sync.lifecycle.state = state;
    if waiting {
        sync.queue.push_back(QueuedCommand {
            command: SourceCommand::OpenSettings,
            focus_token: None,
            deadline_ms: u64::MAX,
            scope: QueueScope::Anywhere,
        });
    }
    sync
}

/// What `submit` must answer in that state. There is no `Busy`: a command is
/// run, or held, or answered as unavailable.
#[derive(Debug, Clone, Copy)]
enum Expect {
    Execute,
    Snapshot,
    WaitingForReady,
    Queued,
    Unavailable,
}

impl Expect {
    fn holds(self, outcome: &SubmitOutcome) -> bool {
        matches!(
            (self, outcome),
            (Expect::Execute, SubmitOutcome::ExecuteQueued)
                | (Expect::Snapshot, SubmitOutcome::SnapshotRequested(_))
                | (Expect::WaitingForReady, SubmitOutcome::WaitingForReady)
                | (Expect::Queued, SubmitOutcome::Queued)
                | (Expect::Unavailable, SubmitOutcome::Unavailable)
        )
    }
}

fn cases() -> Vec<(&'static str, SourceSyncState, Expect)> {
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
    let held_editor = |state| in_state(state, false);
    vec![
        (
            "unmounted",
            in_state(LifecycleState::Unmounted, false),
            Expect::Execute,
        ),
        (
            "mounting, nothing waiting",
            in_state(mounting(), false),
            Expect::WaitingForReady,
        ),
        (
            "mounting, a command waiting",
            in_state(mounting(), true),
            Expect::WaitingForReady,
        ),
        (
            "initializing, nothing waiting",
            in_state(initializing(), false),
            Expect::WaitingForReady,
        ),
        (
            "initializing, a command waiting",
            in_state(initializing(), true),
            Expect::WaitingForReady,
        ),
        (
            "ready",
            in_state(LifecycleState::Ready(ready()), false),
            Expect::Snapshot,
        ),
        (
            "snapshot pending",
            held_editor(LifecycleState::SnapshotPending {
                editor: ready(),
                command: SourceCommand::SaveNow,
                operation: operation(),
            }),
            Expect::Queued,
        ),
        (
            "barrier held",
            held_editor(LifecycleState::BarrierHeld {
                editor: held(),
                command: SourceCommand::SaveNow,
                before: SessionFingerprint {
                    document_id: Some(DOCUMENT),
                    revision: Some(1),
                    source_token: DOCUMENT,
                },
            }),
            Expect::Queued,
        ),
        (
            "resume pending",
            held_editor(LifecycleState::ResumePending {
                editor: held(),
                operation: operation(),
            }),
            Expect::Queued,
        ),
        (
            "refresh pending",
            held_editor(LifecycleState::RefreshPending {
                editor: held(),
                new_epoch: SourceEpoch::new(2),
                revision: 2,
                operation: operation(),
            }),
            Expect::Queued,
        ),
        ("unmounting", held_editor(unmounting(None)), Expect::Queued),
        (
            "unmounting, a mount waiting",
            held_editor(unmounting(Some(intent()))),
            Expect::Queued,
        ),
        (
            "unavailable, nothing retired",
            held_editor(LifecycleState::Unavailable { retired: None }),
            Expect::Execute,
        ),
        (
            "unavailable, retired",
            held_editor(LifecycleState::Unavailable {
                retired: Some(identity()),
            }),
            Expect::Unavailable,
        ),
    ]
}

/// What the accessor must report for each case: a state that holds a command,
/// with `Mounting` and `Initializing` only while a command is already waiting.
fn expected_busy(label: &str) -> Option<&'static str> {
    match label {
        "unmounted"
        | "mounting, nothing waiting"
        | "initializing, nothing waiting"
        | "ready"
        | "unavailable, nothing retired"
        | "unavailable, retired" => None,
        "mounting, a command waiting" => Some("Mounting"),
        "initializing, a command waiting" => Some("Initializing"),
        "snapshot pending" => Some("SnapshotPending"),
        "barrier held" => Some("BarrierHeld"),
        "resume pending" => Some("ResumePending"),
        "refresh pending" => Some("RefreshPending"),
        "unmounting" | "unmounting, a mount waiting" => Some("Unmounting"),
        other => panic!("unclassified case: {other}"),
    }
}

#[test]
fn submit_holds_a_command_exactly_where_the_accessor_reports_a_busy_state() {
    for (label, mut sync, expect) in cases() {
        let reported = sync.busy_lifecycle_state();
        assert_eq!(reported, expected_busy(label), "{label}");
        // A command that is neither a same-mode no-op nor otherwise special.
        let outcome = sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);
        assert!(
            expect.holds(&outcome),
            "{label}: submit answered {outcome:?}, expected {expect:?}"
        );
        if reported.is_some() {
            assert!(
                matches!(
                    outcome,
                    SubmitOutcome::Queued | SubmitOutcome::WaitingForReady
                ),
                "{label}: a state the accessor reports holds the command in the queue"
            );
            assert_eq!(
                sync.queue.len(),
                1 + usize::from(label.contains("a command waiting"))
            );
        }
        assert!(
            !sync.has_discards(),
            "{label}: nothing is dropped on the way in"
        );
    }
}

/// Every state the accessor reports accepts a command; none drops one.
#[test]
fn no_state_answers_busy_and_drops() {
    for (label, mut sync, _) in cases() {
        let before = sync.queue.len();
        let outcome = sync.submit(SourceCommand::SaveNow, Some(DOCUMENT), 10);
        let accepted = matches!(
            outcome,
            SubmitOutcome::ExecuteQueued
                | SubmitOutcome::SnapshotRequested(_)
                | SubmitOutcome::WaitingForReady
                | SubmitOutcome::Queued
                | SubmitOutcome::Unavailable
        );
        assert!(accepted, "{label}: {outcome:?}");
        if matches!(
            outcome,
            SubmitOutcome::Queued | SubmitOutcome::WaitingForReady
        ) {
            assert_eq!(
                sync.queue.len(),
                before + 1,
                "{label}: the command is in the queue"
            );
        }
    }
}

#[test]
fn busy_lifecycle_state_names_the_state_it_reports() {
    for (label, sync, _) in cases() {
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
        .map(|(_, sync, _)| variant_name(&sync.lifecycle.state))
        .collect();
    assert_eq!(covered.len(), 10, "covered: {covered:?}");
}

#[test]
fn the_accessor_changes_nothing() {
    for (label, sync, _) in cases() {
        let before = format!("{:?}", sync.lifecycle.state);
        let waiting = sync.queue.clone();
        let _ = sync.busy_lifecycle_state();
        assert_eq!(before, format!("{:?}", sync.lifecycle.state), "{label}");
        assert_eq!(waiting, sync.queue, "{label}");
    }
}
