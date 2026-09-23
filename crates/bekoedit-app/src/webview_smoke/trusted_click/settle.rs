//! The same settle gate `shell_behaviour.rs` uses (task 021: no exchange
//! is requested while the source controller would answer `Busy`), copied
//! rather than shared -- `shell_behaviour.rs`'s own copy is private to
//! that module, and this run must not touch it (per `trusted_click.rs`'s
//! own doc comment on non-shared scope).
//!
//! Built during task 023's investigation into §C's `ModeTabFocus` phase
//! (since removed to task 024 -- see `phase.rs`'s own doc comment): a
//! bespoke DOM-level eval saw the Text editor as visually ready within a
//! few hundred milliseconds of a mode-switching click, yet the very next
//! phase query still hit the shared transport's 5 s round-trip cap. The
//! DOM can look ready before `SourceSyncState`'s own async lifecycle
//! (mount, snapshot, barrier) has actually finished settling -- the gap
//! this gate, not a DOM check, is built to close. Kept for §B's own
//! remaining phases, even though neither has ever shown this gap in
//! practice, since checking before every exchange is cheap and matches
//! `shell_behaviour.rs`'s own precedent.

use std::time::Duration;

use crate::source_sync::SourceSyncState;
use crate::source_sync::lifecycle::LifecycleState;

use super::phase::TrustedClickPhase;

#[derive(Debug, Clone, Copy)]
pub(super) struct SettleGate {
    pub(super) deadline: Duration,
    pub(super) poll: Duration,
}

/// Every non-`Ready` lifecycle state, unconditionally -- unlike
/// `SourceSyncState::busy_lifecycle_state` (task 021), which only counts
/// `Mounting`/`Initializing` as busy when a command is *queued* behind
/// them, because that check exists to answer a narrower question: is a
/// queued command blocked. Task 023's now-removed `ModeTabFocus` phase
/// (see `phase.rs`'s own doc comment) mounted and unmounted the Text
/// editor through two real clicks with nothing ever queued behind them,
/// so `busy_lifecycle_state` reported clear immediately while the mount
/// was, in fact, still in flight -- invisible to that narrower predicate
/// by design, not a bug in it.
pub(super) fn unsettled_lifecycle_state(sync: &SourceSyncState) -> Option<String> {
    match &sync.lifecycle.state {
        LifecycleState::Unmounted
        | LifecycleState::Ready(_)
        | LifecycleState::Unavailable { .. } => None,
        LifecycleState::Mounting { .. } => Some("Mounting".to_string()),
        LifecycleState::Initializing { .. } => Some("Initializing".to_string()),
        LifecycleState::SnapshotPending { .. } => Some("SnapshotPending".to_string()),
        LifecycleState::BarrierHeld { .. } => Some("BarrierHeld".to_string()),
        LifecycleState::ResumePending { .. } => Some("ResumePending".to_string()),
        LifecycleState::RefreshPending { .. } => Some("RefreshPending".to_string()),
        LifecycleState::Unmounting { .. } => Some("Unmounting".to_string()),
    }
}

pub(super) const SETTLE_GATE: SettleGate = SettleGate {
    deadline: Duration::from_secs(10),
    poll: Duration::from_millis(20),
};

pub(super) async fn wait_until_settled(
    phase: TrustedClickPhase,
    gate: SettleGate,
    mut busy_state: impl FnMut() -> Option<String>,
) -> Result<(), String> {
    let started = tokio::time::Instant::now();
    loop {
        let Some(state) = busy_state() else {
            return Ok(());
        };
        if started.elapsed() >= gate.deadline {
            return Err(format!(
                "the source controller did not settle before the {} exchange: \
                 still {state} after {:?}",
                phase.as_str(),
                gate.deadline
            ));
        }
        tokio::time::sleep(gate.poll).await;
    }
}

#[cfg(test)]
mod tests {
    use bekoedit_ui_contract::source_editor::{
        EditorIdentity, EditorInstanceId, OperationId, SourceEditorId, SourceEpoch,
    };

    use crate::source_sync::lifecycle::PendingOperation;

    use super::*;

    fn identity() -> EditorIdentity {
        EditorIdentity {
            instance_id: EditorInstanceId::new(1),
            editor_id: SourceEditorId::Text,
            document_id: 7,
            epoch: SourceEpoch::new(1),
        }
    }

    #[test]
    fn unsettled_lifecycle_state_reports_mounting_even_with_an_empty_queue() {
        // The distinguishing behaviour from busy_lifecycle_state (task
        // 021): that check reports Mounting/Initializing as busy only
        // when a command is queued behind them. This run never queues
        // anything -- its two real clicks reach the controller directly
        // -- so it needs a check that reports Mounting regardless.
        let mut sync = SourceSyncState::default();
        sync.lifecycle.state = LifecycleState::Initializing {
            identity: identity(),
            revision: 1,
            operation: PendingOperation {
                operation_id: OperationId::new(1),
                deadline_ms: u64::MAX,
            },
        };
        assert_eq!(
            sync.busy_lifecycle_state(),
            None,
            "queue-gated: nothing is queued behind it, so not busy"
        );
        assert_eq!(
            unsettled_lifecycle_state(&sync),
            Some("Initializing".to_string()),
            "unconditional: still mounting"
        );
    }

    #[test]
    fn unsettled_lifecycle_state_is_clear_once_ready() {
        assert_eq!(unsettled_lifecycle_state(&SourceSyncState::default()), None);
    }

    fn quick_gate(deadline_ms: u64) -> SettleGate {
        SettleGate {
            deadline: Duration::from_millis(deadline_ms),
            poll: Duration::from_millis(2),
        }
    }

    #[tokio::test]
    async fn a_settled_controller_is_not_delayed() {
        let mut calls = 0_u32;
        wait_until_settled(TrustedClickPhase::ProofOfTrust, quick_gate(5_000), || {
            calls += 1;
            None
        })
        .await
        .unwrap();
        assert_eq!(calls, 1, "returns on the first, immediately clear read");
    }

    #[tokio::test]
    async fn a_controller_that_clears_partway_through_is_allowed() {
        let mut calls = 0_u32;
        wait_until_settled(TrustedClickPhase::TreeRowFocus, quick_gate(5_000), || {
            calls += 1;
            (calls < 3).then(|| "Mounting".to_string())
        })
        .await
        .unwrap();
        assert_eq!(calls, 3, "polled until the state cleared, not just once");
    }

    #[tokio::test]
    async fn a_controller_that_never_settles_fails_naming_the_phase_and_the_state() {
        let error = wait_until_settled(TrustedClickPhase::BacklinkFocus, quick_gate(30), || {
            Some("Unmounting".to_string())
        })
        .await
        .unwrap_err();
        assert!(error.contains("backlink_focus"), "{error}");
        assert!(error.contains("Unmounting"), "{error}");
        assert!(error.contains("did not settle"), "{error}");
    }
}
