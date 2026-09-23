//! A small module of its own so `tests.rs` stays under the ELOC guideline
//! (task 025 §5, mirroring `phase_bijection.rs`'s own reason). Task 025
//! §2.3/§4.3: RFC-044's `ShellBehaviourMachine::validate` now surfaces a
//! terminal failure's own reason through every structural rejection --
//! the same property task 023 review (root-cause-fixed) §2 found missing
//! and fixed for `trusted_click.rs`'s own validator, generalised here via
//! the shared `transport::reject_with_reason` helper.

use crate::webview_smoke::shell_behaviour::ShellBehaviourMachine;
use crate::webview_smoke::shell_behaviour::phase::ShellBehaviourPhase;
use crate::webview_smoke::transport::{
    DriverResult, MessageKind, PhaseMessage, PinnedExchange, SMOKE_PROTOCOL_VERSION,
};

/// Exactly the shape a `failEarly`-style pre-try rejection sends: a
/// terminal failure whose `releasedExchangeId` is always `null`, since the
/// driver has no way to know what Rust actually expected released -- which
/// used to trigger the pin-release check's own generic complaint,
/// discarding the driver's more specific reason (task 023's finding).
#[test]
fn a_terminal_failure_s_own_reason_survives_a_structural_pin_mismatch() {
    let machine = ShellBehaviourMachine::for_phase(ShellBehaviourPhase::DownUp);
    let message = PhaseMessage {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id: 6,
        kind: MessageKind::Terminal,
        phase: "down_up".into(),
        released_exchange_id: None,
        released_phase: None,
        milestone: None,
        result: Some(DriverResult {
            ok: false,
            stage: "invalid_request".into(),
            marker: "RFC044_SHELL_BEHAVIOUR_MARKER".into(),
            milestones: Vec::new(),
            error_toast_seen: false,
            error: Some("invalid phase request: phase=\"garbage\"".into()),
        }),
    };
    let release = PinnedExchange {
        exchange_id: 5,
        phase: ShellBehaviourPhase::RecoveryExit,
    };

    let error = machine.validate(&message, 6, Some(release)).unwrap_err();
    assert!(
        error.contains("invalid phase request"),
        "the driver's own reason must survive a structural rejection: {error}"
    );
}
