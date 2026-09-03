use super::*;
use crate::webview_smoke::transport::{MessageKind, PhaseMessage, SMOKE_PROTOCOL_VERSION};

fn phase_message(kind: MessageKind, phase: &str, exchange_id: u64) -> PhaseMessage {
    PhaseMessage {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id,
        kind,
        phase: phase.into(),
        released_exchange_id: None,
        released_phase: None,
        milestone: None,
        result: None,
    }
}

fn successful_result() -> DriverResult {
    DriverResult {
        ok: true,
        stage: "enter_opens".into(),
        marker: MARKER.into(),
        milestones: EXPECTED_MILESTONES
            .iter()
            .map(|item| (*item).into())
            .collect(),
        error_toast_seen: false,
        error: None,
    }
}

#[test]
fn machine_advances_through_all_six_phases_on_valid_progress() {
    let mut machine = ShellBehaviourMachine::new();
    let progression = [
        (
            ShellBehaviourPhase::DownUp,
            "down_up_moved",
            ShellBehaviourPhase::ExpandEnter,
        ),
        (
            ShellBehaviourPhase::ExpandEnter,
            "expand_entered",
            ShellBehaviourPhase::CollapseAscend,
        ),
        (
            ShellBehaviourPhase::CollapseAscend,
            "collapse_ascended",
            ShellBehaviourPhase::HomeEnd,
        ),
        (
            ShellBehaviourPhase::HomeEnd,
            "home_end_reached",
            ShellBehaviourPhase::NonOpenable,
        ),
        (
            ShellBehaviourPhase::NonOpenable,
            "non_openable_reachable",
            ShellBehaviourPhase::EnterOpens,
        ),
    ];
    for (index, (phase, milestone, next)) in progression.into_iter().enumerate() {
        let exchange_id = (index + 1) as u64;
        assert_eq!(machine.current(), phase);
        let mut message = phase_message(MessageKind::Progress, phase.as_str(), exchange_id);
        message.milestone = Some(milestone.into());
        machine.validate(&message, exchange_id, None).unwrap();
        machine.apply_completed(exchange_id, &message).unwrap();
        assert_eq!(machine.current(), next);
    }
}

#[test]
fn terminal_can_come_from_any_phase_not_only_the_last_one() {
    // Regression test: a driver's try/catch turns a thrown error into a
    // terminal *failure* at whichever phase raised it -- exactly as
    // driver.js's own three phases each can. An earlier version of this
    // validate() incorrectly restricted Terminal to EnterOpens only,
    // which meant a real down_up failure surfaced as "only enter_opens can
    // return a terminal result" instead of the driver's actual error
    // (caught by CI on the first real run against a WebView, 2026-09-04).
    let machine = ShellBehaviourMachine::new();
    assert_eq!(machine.current(), ShellBehaviourPhase::DownUp);
    let mut message = phase_message(MessageKind::Terminal, "down_up", 1);
    let mut failed = successful_result();
    failed.ok = false;
    failed.stage = "down_up".into();
    failed.error = Some("could not focus the first tree row directly".into());
    message.result = Some(failed);
    machine.validate(&message, 1, None).unwrap();
}

#[test]
fn malformed_progress_and_terminal_messages_are_rejected() {
    let machine = ShellBehaviourMachine::new();

    let mut wrong_milestone = phase_message(MessageKind::Progress, "down_up", 1);
    wrong_milestone.milestone = Some("expand_entered".into());
    assert!(machine.validate(&wrong_milestone, 1, None).is_err());

    let progress_with_result = {
        let mut message = phase_message(MessageKind::Progress, "down_up", 1);
        message.milestone = Some("down_up_moved".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&progress_with_result, 1, None).is_err());

    let terminal_with_milestone = {
        let mut message = phase_message(MessageKind::Terminal, "down_up", 1);
        message.milestone = Some("down_up_moved".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&terminal_with_milestone, 1, None).is_err());

    let terminal_without_result = phase_message(MessageKind::Terminal, "down_up", 1);
    assert!(machine.validate(&terminal_without_result, 1, None).is_err());

    let out_of_order = phase_message(MessageKind::Pending, "expand_enter", 1);
    assert!(machine.validate(&out_of_order, 1, None).is_err());

    let last_phase_terminal_progress =
        ShellBehaviourMachine::for_phase(ShellBehaviourPhase::EnterOpens);
    let mut malformed = phase_message(MessageKind::Progress, "enter_opens", 1);
    malformed.milestone = Some("enter_opened_editor_focused".into());
    assert!(
        last_phase_terminal_progress
            .validate(&malformed, 1, None)
            .is_err(),
        "enter_opens cannot return nonterminal progress"
    );
}

#[test]
fn validate_result_checks_stage_marker_toast_and_milestones() {
    assert!(validate_shell_behaviour_result(&successful_result()).is_ok());

    let mutations: [fn(&mut DriverResult); 5] = [
        |result| result.stage = "down_up".into(),
        |result| result.marker = "wrong".into(),
        |result| result.error_toast_seen = true,
        |result| result.error = Some("contradictory success".into()),
        |result| {
            result.milestones.swap(0, 1);
        },
    ];
    for mutate in mutations {
        let mut result = successful_result();
        mutate(&mut result);
        assert!(validate_shell_behaviour_result(&result).is_err());
    }

    let mut failed = successful_result();
    failed.ok = false;
    failed.error = Some("explicit failure".into());
    assert!(validate_shell_behaviour_result(&failed).is_err());
}

#[test]
fn terminal_result_transitions_the_terminal_exactly_once() {
    let terminal = ShellBehaviourTerminal::default();
    assert!(!terminal.succeeded());
    assert!(terminal.accept(&successful_result()).is_ok());
    assert!(terminal.succeeded());
    assert!(
        terminal.accept(&successful_result()).is_err(),
        "duplicates are rejected"
    );
    assert!(terminal.succeeded(), "late results cannot reverse success");
}
