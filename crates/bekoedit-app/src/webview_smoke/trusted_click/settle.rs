//! The same settle gate `shell_behaviour.rs` uses (task 021: no exchange
//! is requested while the source controller would answer `Busy`), copied
//! rather than shared -- `shell_behaviour.rs`'s own copy is private to
//! that module, and this run must not touch it (per `trusted_click.rs`'s
//! own doc comment on non-shared scope).
//!
//! CI's eighth real run showed why this run needs it too, not just a
//! DOM-level wait: `xtest::await_editor_settled`'s bespoke eval saw the
//! Text editor as visually ready within a few hundred milliseconds of the
//! mode-text click, yet the very next phase query still hit the shared
//! transport's 5 s round-trip cap. The DOM can look ready before
//! `SourceSyncState`'s own async lifecycle (mount, snapshot, barrier) has
//! actually finished settling -- exactly the gap this gate, not a DOM
//! check, is built to close.

use std::time::Duration;

use super::phase::TrustedClickPhase;

#[derive(Debug, Clone, Copy)]
pub(super) struct SettleGate {
    pub(super) deadline: Duration,
    pub(super) poll: Duration,
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
    use super::*;

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
        let error = wait_until_settled(TrustedClickPhase::ModeTabFocus, quick_gate(30), || {
            Some("Unmounting".to_string())
        })
        .await
        .unwrap_err();
        assert!(
            error.contains("mode_tab_trusted_click_focused_editor"),
            "{error}"
        );
        assert!(error.contains("Unmounting"), "{error}");
        assert!(error.contains("did not settle"), "{error}");
    }
}
