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
        stage: TERMINAL_STAGE.into(),
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
fn machine_advances_through_every_transition_ending_at_mode_tab_focus() {
    let mut machine = TrustedClickMachine::new();
    let progression = [
        (
            TrustedClickPhase::ProofOfTrust,
            "trusted_click_focused_default_target",
            TrustedClickPhase::TreeRowFocus,
        ),
        (
            TrustedClickPhase::TreeRowFocus,
            "tree_row_trusted_click_focused_editor",
            TrustedClickPhase::BacklinkFocus,
        ),
        (
            TrustedClickPhase::BacklinkFocus,
            "backlink_trusted_click_focused_editor",
            TrustedClickPhase::ModeTabFocus,
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
    assert_eq!(
        TrustedClickPhase::ModeTabFocus.next(),
        None,
        "mode_tab_focus (§C, the check this file exists for) is the terminal phase"
    );
    assert_eq!(TrustedClickPhase::ModeTabFocus.as_str(), "mode_tab_focus");
}

/// The 2026-09-23 finding review's root cause, made structural: every
/// `TrustedClickPhase::as_str()` value must be a phase name
/// `trusted_click_driver.js`'s own `phases` array recognizes. Before this
/// test existed, the terminal phase's `as_str()` returned `TERMINAL_STAGE`
/// (a result-*stage* name, not a phase name) on every one of sixteen real
/// CI runs across this branch's whole history -- a request the driver
/// rejected before its own `try`/`catch`, with nothing ever sent back, so
/// Rust silently waited out the shared transport's 5 s cap every time.
#[test]
fn every_as_str_is_a_phase_the_driver_knows() {
    let driver_phases: std::collections::BTreeSet<&str> = TRUSTED_CLICK_JS
        .split_once("const phases = [")
        .expect("driver must declare its phases array")
        .1
        .split_once(']')
        .expect("phases array must be closed")
        .0
        .split(',')
        .map(|entry| entry.trim().trim_matches('"'))
        .filter(|entry| !entry.is_empty())
        .collect();
    let rust_phases: std::collections::BTreeSet<&str> = [
        TrustedClickPhase::ProofOfTrust,
        TrustedClickPhase::TreeRowFocus,
        TrustedClickPhase::BacklinkFocus,
        TrustedClickPhase::ModeTabFocus,
    ]
    .into_iter()
    .map(TrustedClickPhase::as_str)
    .collect();
    assert_eq!(
        rust_phases, driver_phases,
        "TrustedClickPhase::as_str() must match trusted_click_driver.js's own \
         phases array exactly -- a mismatch is a request the driver rejects \
         before its own try/catch, silently, for the shared transport's full \
         5 s round-trip cap"
    );
}

#[test]
fn terminal_can_come_from_any_phase_not_only_the_last_one() {
    // Same regression this run's sibling runs already guard against
    // (shell_behaviour/tests.rs, webview_smoke/tests.rs): a driver's
    // try/catch turns a thrown error into a terminal *failure* at
    // whichever phase raised it.
    let machine = TrustedClickMachine::new();
    assert_eq!(machine.current(), TrustedClickPhase::ProofOfTrust);
    let mut message = phase_message(MessageKind::Terminal, "proof_of_trust", 1);
    let mut failed = successful_result();
    failed.ok = false;
    failed.stage = "proof_of_trust".into();
    failed.error = Some("xdotool exited with a nonzero status".into());
    message.result = Some(failed);
    machine.validate(&message, 1, None).unwrap();
}

#[test]
fn malformed_progress_and_terminal_messages_are_rejected() {
    let machine = TrustedClickMachine::for_phase(TrustedClickPhase::TreeRowFocus);

    let mut wrong_milestone = phase_message(MessageKind::Progress, "tree_row_focus", 1);
    wrong_milestone.milestone = Some("backlink_trusted_click_focused_editor".into());
    assert!(machine.validate(&wrong_milestone, 1, None).is_err());

    let progress_with_result = {
        let mut message = phase_message(MessageKind::Progress, "tree_row_focus", 1);
        message.milestone = Some("tree_row_trusted_click_focused_editor".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&progress_with_result, 1, None).is_err());

    let terminal_with_milestone = {
        let mut message = phase_message(MessageKind::Terminal, "tree_row_focus", 1);
        message.milestone = Some("tree_row_trusted_click_focused_editor".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&terminal_with_milestone, 1, None).is_err());

    let terminal_without_result = phase_message(MessageKind::Terminal, "tree_row_focus", 1);
    assert!(machine.validate(&terminal_without_result, 1, None).is_err());

    let out_of_order = phase_message(MessageKind::Pending, "backlink_focus", 1);
    assert!(machine.validate(&out_of_order, 1, None).is_err());

    let terminal_phase_nonterminal_progress =
        TrustedClickMachine::for_phase(TrustedClickPhase::ModeTabFocus);
    let mut malformed = phase_message(MessageKind::Progress, "mode_tab_focus", 1);
    malformed.milestone = Some(TERMINAL_STAGE.into());
    assert!(
        terminal_phase_nonterminal_progress
            .validate(&malformed, 1, None)
            .is_err(),
        "the terminal phase cannot return nonterminal progress"
    );
}

#[test]
fn released_pin_must_match_the_prior_exchange_exactly() {
    let machine = TrustedClickMachine::for_phase(TrustedClickPhase::TreeRowFocus);
    let mut message = phase_message(MessageKind::Progress, "tree_row_focus", 2);
    message.milestone = Some("tree_row_trusted_click_focused_editor".into());
    message.released_exchange_id = Some(1);
    message.released_phase = Some("proof_of_trust".into());

    let release = PinnedExchange {
        exchange_id: 1,
        phase: TrustedClickPhase::ProofOfTrust,
    };
    assert!(machine.validate(&message, 2, Some(release)).is_ok());

    let wrong_release = PinnedExchange {
        exchange_id: 1,
        phase: TrustedClickPhase::TreeRowFocus,
    };
    assert!(machine.validate(&message, 2, Some(wrong_release)).is_err());
    assert!(machine.validate(&message, 2, None).is_err());
}

#[test]
fn validate_result_checks_stage_marker_toast_and_milestones() {
    assert!(validate_trusted_click_result(&successful_result()).is_ok());

    let mutations: [fn(&mut DriverResult); 5] = [
        |result| result.stage = "tree_row_focus".into(),
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
        assert!(validate_trusted_click_result(&result).is_err());
    }

    let mut failed = successful_result();
    failed.ok = false;
    failed.error = Some("explicit failure".into());
    assert!(validate_trusted_click_result(&failed).is_err());
}

#[test]
fn terminal_result_transitions_the_terminal_exactly_once() {
    let terminal = TrustedClickTerminal::default();
    assert!(!terminal.succeeded());
    assert!(terminal.accept(&successful_result()).is_ok());
    assert!(terminal.succeeded());
    assert!(
        terminal.accept(&successful_result()).is_err(),
        "duplicates are rejected"
    );
    assert!(terminal.succeeded(), "late results cannot reverse success");
}

fn scratch_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bekoedit-webview-smoke-test-{label}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn prepare_seeds_a_linked_pair_and_opens_neither() {
    let root = scratch_dir("trusted-click-prepare");
    let prepared = prepare(&root).unwrap();
    let workspace = prepared
        .persistence
        .isolated_paths()
        .expect("isolated persistence")
        .root()
        .join("workspace");

    let child = std::fs::read_to_string(workspace.join("child.md")).unwrap();
    assert_eq!(child, "# child\n");
    let parent = std::fs::read_to_string(workspace.join("parent.md")).unwrap();
    assert!(
        parent.to_lowercase().contains("child.md"),
        "parent.md must link to child.md for find_backlinks to report it: {parent}"
    );

    let backlinks = bekoedit_fs::find_backlinks(&workspace, std::path::Path::new("child.md"));
    assert_eq!(backlinks.len(), 1, "exactly one backlink: parent.md");
    assert_eq!(backlinks[0].source_path, PathBuf::from("parent.md"));

    let settings = prepared.persistence.load_settings();
    assert!(settings.reopen_last_workspace);
    assert_eq!(settings.default_mode, EditorMode::Text);

    std::fs::remove_dir_all(&prepared.root).unwrap();
}
