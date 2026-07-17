use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::rc::Rc;

use super::*;

fn successful_result() -> DriverResult {
    DriverResult {
        ok: true,
        stage: "preview_verified".into(),
        marker: MARKER.into(),
        milestones: EXPECTED_MILESTONES
            .iter()
            .map(|item| (*item).into())
            .collect(),
        error_toast_seen: false,
        error: None,
    }
}

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

fn phase_completion(kind: MessageKind, phase: &str, exchange_id: u64) -> PhaseCompletion {
    PhaseCompletion {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id,
        phase: phase.into(),
        kind,
        acknowledgement_processed: true,
        evaluator_pinned: true,
    }
}

#[test]
fn run_mode_requires_exact_webview_profile_argument() {
    assert_eq!(
        RunMode::parse(Vec::<OsString>::new()).unwrap(),
        RunMode::Normal
    );
    assert_eq!(
        RunMode::parse([OsString::from("--headless-smoke")]).unwrap(),
        RunMode::HeadlessSmoke
    );
    assert!(RunMode::parse([OsString::from("--webview-smoke")]).is_err());
    let path = PathBuf::from("/tmp/bekoedit-webview-smoke-test");
    assert_eq!(
        RunMode::parse([
            OsString::from("--webview-smoke"),
            path.clone().into_os_string()
        ])
        .unwrap(),
        RunMode::WebViewSmoke(path)
    );
}

#[test]
fn profile_rejects_relative_wrong_named_and_existing_roots() {
    assert!(SmokeProfile::create(Path::new("relative")).is_err());
    let parent = tempfile::tempdir().unwrap();
    assert!(SmokeProfile::create(&parent.path().join("unsafe-name")).is_err());
    let existing = parent.path().join("bekoedit-webview-smoke-existing");
    std::fs::create_dir(&existing).unwrap();
    assert!(SmokeProfile::create(&existing).is_err());
}

#[test]
fn profile_creates_owned_isolated_paths_and_cleanup_is_bounded() {
    let parent = tempfile::tempdir().unwrap();
    let requested = parent.path().join("bekoedit-webview-smoke-owned");
    let profile = SmokeProfile::create(&requested).unwrap();
    let paths = profile.persistence.isolated_paths().unwrap();
    assert!(paths.all_within_root());
    assert!(paths.settings_file().parent().unwrap().exists());
    assert!(paths.recovery_dir().exists());
    assert!(paths.history_dir().exists());
    let root = profile.root;
    std::fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
    assert!(parent.path().exists());
}

#[test]
fn terminal_is_fail_closed_and_only_exact_success_transitions_once() {
    let terminal = SmokeTerminal::default();
    assert!(!terminal.succeeded(), "no result and unexpected close fail");

    let mut failed = successful_result();
    failed.ok = false;
    failed.error = Some("explicit failure".into());
    assert!(terminal.accept(&failed).is_err());
    assert!(!terminal.succeeded());

    let success = successful_result();
    assert!(terminal.accept(&success).is_ok());
    assert!(terminal.succeeded());
    assert!(
        terminal.accept(&success).is_err(),
        "duplicates are rejected"
    );
    assert!(terminal.succeeded(), "late results cannot reverse success");
}

#[test]
fn smoke_run_maps_no_result_and_exact_success_to_process_codes() {
    let parent = tempfile::tempdir().unwrap();
    let failed_root = parent.path().join("bekoedit-webview-smoke-no-result");
    std::fs::create_dir(&failed_root).unwrap();
    let failed = SmokeRun {
        profile_root: Some(failed_root.clone()),
        terminal: Arc::new(SmokeTerminal::default()),
    };
    assert_eq!(failed.finalize_exit_code(), 1);
    assert!(!failed_root.exists());

    let passed_root = parent.path().join("bekoedit-webview-smoke-success");
    std::fs::create_dir(&passed_root).unwrap();
    let terminal = Arc::new(SmokeTerminal::default());
    terminal.accept(&successful_result()).unwrap();
    let passed = SmokeRun {
        profile_root: Some(passed_root.clone()),
        terminal,
    };
    assert_eq!(passed.finalize_exit_code(), 0);
    assert!(!passed_root.exists());
}

#[test]
fn smoke_run_drop_cleans_profile_before_event_loop_initialization() {
    let parent = tempfile::tempdir().unwrap();
    let profile_root = parent.path().join("bekoedit-webview-smoke-init-panic");
    std::fs::create_dir(&profile_root).unwrap();
    let run = SmokeRun {
        profile_root: Some(profile_root.clone()),
        terminal: Arc::new(SmokeTerminal::default()),
    };
    drop(run);
    assert!(!profile_root.exists());
    assert!(parent.path().exists());
}

#[test]
fn terminal_rejects_wrong_order_marker_stage_and_transient_error() {
    let mutations = [
        |result: &mut DriverResult| result.milestones.swap(0, 1),
        |result: &mut DriverResult| result.marker = "wrong".into(),
        |result: &mut DriverResult| result.stage = "preview_clicked".into(),
        |result: &mut DriverResult| result.error_toast_seen = true,
        |result: &mut DriverResult| result.error = Some("contradictory success".into()),
    ];
    for mutate in mutations {
        let terminal = SmokeTerminal::default();
        let mut result = successful_result();
        mutate(&mut result);
        assert!(terminal.accept(&result).is_err());
        assert!(!terminal.succeeded());
    }
}

#[test]
fn driver_contract_is_bounded_observable_and_uses_rendered_controls() {
    assert!(WEBVIEW_SMOKE_JS.starts_with("return (async () =>"));
    assert!(WEBVIEW_SMOKE_JS.contains("performance.now() + 15000"));
    assert!(WEBVIEW_SMOKE_JS.contains("MutationObserver"));
    assert!(WEBVIEW_SMOKE_JS.contains("errorToastSeen"));
    assert!(WEBVIEW_SMOKE_JS.contains("start-new"));
    assert!(WEBVIEW_SMOKE_JS.contains("view.hasFocus"));
    assert!(WEBVIEW_SMOKE_JS.contains("view.dispatch"));
    assert!(WEBVIEW_SMOKE_JS.contains("mode-preview"));
    assert!(WEBVIEW_SMOKE_JS.contains("article.preview"));
    assert!(WEBVIEW_SMOKE_JS.contains("preview_verified"));
    assert!(WEBVIEW_SMOKE_JS.contains("await dioxus.recv()"));
    assert!(WEBVIEW_SMOKE_JS.contains("__bkWebViewSmokeEvalPin"));
    assert!(WEBVIEW_SMOKE_JS.contains("channel: dioxus"));
    assert!(WEBVIEW_SMOKE_JS.contains("acknowledgementProcessed: true"));
    assert!(WEBVIEW_SMOKE_JS.contains("evaluatorPinned: true"));

    let release = WEBVIEW_SMOKE_JS
        .find("pinRegistry.current = null")
        .expect("prior pin release must exist");
    let first_dom_read = WEBVIEW_SMOKE_JS
        .find("document.querySelector")
        .expect("driver must use rendered controls");
    assert!(
        release < first_dom_read,
        "pin release must precede DOM work"
    );

    let pin = WEBVIEW_SMOKE_JS
        .find("pinRegistry.current = Object.freeze")
        .expect("channel pin must exist");
    let completion = WEBVIEW_SMOKE_JS
        .find("acknowledgementProcessed: true")
        .expect("typed completion must exist");
    assert!(pin < completion, "channel must be pinned before return");

    let rust = include_str!("../webview_smoke.rs");
    assert!(rust.contains("tokio::time::timeout_at(deadline"));
    assert!(rust.contains("eval.join::<PhaseCompletion>()"));
    assert!(!rust.contains("let mut eval = document::eval(WEBVIEW_SMOKE_JS);\n            loop"));
}

#[test]
fn phase_machine_advances_only_after_valid_completed_progress() {
    let mut machine = PhaseMachine::new();
    let mut launch = phase_message(MessageKind::Progress, "launch", 1);
    launch.milestone = Some("new_clicked".into());

    machine.validate(&launch, 1, None).unwrap();
    assert_eq!(
        machine.current,
        SmokePhase::Launch,
        "a reported but never-completed evaluator cannot advance"
    );
    machine.apply_completed(1, &launch).unwrap();
    assert_eq!(machine.current, SmokePhase::Editor);
    assert!(
        machine.apply_completed(1, &launch).is_err(),
        "duplicate completed progress is rejected"
    );

    let mut editor = phase_message(MessageKind::Progress, "editor", 2);
    editor.released_exchange_id = Some(1);
    editor.released_phase = Some("launch".into());
    editor.milestone = Some("preview_clicked".into());
    machine
        .validate(
            &editor,
            2,
            Some(PinnedExchange {
                exchange_id: 1,
                phase: SmokePhase::Launch,
            }),
        )
        .unwrap();
    machine.apply_completed(2, &editor).unwrap();
    assert_eq!(machine.current, SmokePhase::Preview);
    assert!(
        machine.apply_completed(1, &launch).is_err(),
        "stale completion is rejected"
    );
}

#[test]
fn phase_machine_rejects_out_of_order_and_malformed_messages() {
    let machine = PhaseMachine::new();
    assert!(
        machine
            .validate(&phase_message(MessageKind::Pending, "editor", 1), 1, None)
            .is_err()
    );

    let mut malformed = phase_message(MessageKind::Progress, "launch", 1);
    malformed.milestone = Some("preview_clicked".into());
    assert!(machine.validate(&malformed, 1, None).is_err());

    let malformed_terminal = phase_message(MessageKind::Terminal, "launch", 1);
    assert!(machine.validate(&malformed_terminal, 1, None).is_err());
    assert_eq!(machine.current, SmokePhase::Launch);
}

#[test]
fn report_requires_exact_current_exchange_and_prior_pin_release() {
    let machine = PhaseMachine::new();
    let release = PinnedExchange {
        exchange_id: 40,
        phase: SmokePhase::Preview,
    };
    let mut message = phase_message(MessageKind::Pending, "launch", 41);
    message.released_exchange_id = Some(40);
    message.released_phase = Some("preview".into());
    machine.validate(&message, 41, Some(release)).unwrap();

    let mutations = [
        |message: &mut PhaseMessage| message.protocol_version += 1,
        |message: &mut PhaseMessage| message.exchange_id += 1,
        |message: &mut PhaseMessage| message.released_exchange_id = None,
        |message: &mut PhaseMessage| message.released_phase = Some("editor".into()),
    ];
    for mutate in mutations {
        let mut invalid = message.clone();
        mutate(&mut invalid);
        assert!(machine.validate(&invalid, 41, Some(release)).is_err());
    }
}

#[test]
fn completion_requires_exact_correlation_acknowledgement_and_pin() {
    let exact = phase_completion(MessageKind::Progress, "editor", 7);
    validate_completion(&exact, 7, "editor", MessageKind::Progress).unwrap();

    let mutations = [
        |completion: &mut PhaseCompletion| completion.protocol_version += 1,
        |completion: &mut PhaseCompletion| completion.exchange_id += 1,
        |completion: &mut PhaseCompletion| completion.phase = "preview".into(),
        |completion: &mut PhaseCompletion| completion.kind = MessageKind::Pending,
        |completion: &mut PhaseCompletion| completion.acknowledgement_processed = false,
        |completion: &mut PhaseCompletion| completion.evaluator_pinned = false,
    ];
    for mutate in mutations {
        let mut invalid = exact.clone();
        mutate(&mut invalid);
        assert!(validate_completion(&invalid, 7, "editor", MessageKind::Progress).is_err());
    }
}

#[derive(Debug)]
struct ModeledEvaluatorOwner {
    live: bool,
    pinned: bool,
    returned: bool,
}

impl ModeledEvaluatorOwner {
    fn returned(pinned: bool) -> Self {
        Self {
            live: true,
            pinned,
            returned: true,
        }
    }

    fn finalizer_attempt(&mut self) {
        if !self.pinned {
            self.live = false;
        }
    }

    fn consume_join(&self) -> Result<(), &'static str> {
        (self.returned && self.live)
            .then_some(())
            .ok_or("EvalError::Finished")
    }

    fn release(&mut self) {
        self.pinned = false;
    }
}

#[test]
fn modeled_channel_pin_retains_owner_until_join_then_releases() {
    let mut pinned = ModeledEvaluatorOwner::returned(true);
    pinned.finalizer_attempt();
    assert!(pinned.live, "strong JS pin defers query drop");
    pinned.consume_join().unwrap();
    assert!(pinned.returned, "the retained evaluator is not executing");
    pinned.release();
    pinned.finalizer_attempt();
    assert!(!pinned.live, "next probe release permits finalization");

    let mut unpinned = ModeledEvaluatorOwner::returned(false);
    unpinned.finalizer_attempt();
    assert_eq!(unpinned.consume_join(), Err("EvalError::Finished"));
}

#[tokio::test]
async fn real_sequence_waits_between_joined_phase_application_and_next_probe() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let probe_events = events.clone();
    let window_events = events.clone();
    let terminal = SmokeTerminal::default();

    let result = run_smoke_sequence(
        &terminal,
        move |phase, exchange_id, release| {
            probe_events
                .borrow_mut()
                .push(format!("joined:{}", phase.as_str()));
            let (kind, milestone, result) = match phase {
                SmokePhase::Launch => (MessageKind::Progress, Some("new_clicked"), None),
                SmokePhase::Editor => (MessageKind::Progress, Some("preview_clicked"), None),
                SmokePhase::Preview => (MessageKind::Terminal, None, Some(successful_result())),
            };
            let mut message = phase_message(kind, phase.as_str(), exchange_id);
            message.milestone = milestone.map(str::to_string);
            message.result = result;
            if let Some(release) = release {
                message.released_exchange_id = Some(release.exchange_id);
                message.released_phase = Some(release.phase.as_str().into());
            }
            std::future::ready(Ok(CompletedProbe {
                message,
                completion: phase_completion(kind, phase.as_str(), exchange_id),
                pin: PinnedExchange { exchange_id, phase },
            }))
        },
        move || {
            window_events.borrow_mut().push("production_window".into());
            std::future::ready(())
        },
    )
    .await
    .unwrap();

    assert_eq!(result.stage, "preview_verified");
    assert!(terminal.succeeded());
    assert_eq!(
        *events.borrow(),
        [
            "joined:launch",
            "production_window",
            "joined:editor",
            "production_window",
            "joined:preview",
        ]
    );
}

#[tokio::test]
async fn failed_completion_cannot_enter_production_window_or_success() {
    let windows = Rc::new(Cell::new(0));
    let observed_windows = windows.clone();
    let terminal = SmokeTerminal::default();
    let result = run_smoke_sequence(
        &terminal,
        |_, _, _| std::future::ready(Err("joined completion timed out".into())),
        move || {
            observed_windows.set(observed_windows.get() + 1);
            std::future::ready(())
        },
    )
    .await;

    assert_eq!(result.unwrap_err(), "joined completion timed out");
    assert_eq!(windows.get(), 0);
    assert!(!terminal.succeeded());
}

#[tokio::test]
async fn invalid_joined_completion_cannot_enter_production_window_or_success() {
    let windows = Rc::new(Cell::new(0));
    let observed_windows = windows.clone();
    let terminal = SmokeTerminal::default();
    let result = run_smoke_sequence(
        &terminal,
        |phase, exchange_id, _| {
            let mut message = phase_message(MessageKind::Progress, phase.as_str(), exchange_id);
            message.milestone = Some("new_clicked".into());
            let mut completion =
                phase_completion(MessageKind::Progress, phase.as_str(), exchange_id);
            completion.evaluator_pinned = false;
            std::future::ready(Ok(CompletedProbe {
                message,
                completion,
                pin: PinnedExchange { exchange_id, phase },
            }))
        },
        move || {
            observed_windows.set(observed_windows.get() + 1);
            std::future::ready(())
        },
    )
    .await;

    assert!(result.unwrap_err().contains("invalid pinned completion"));
    assert_eq!(windows.get(), 0);
    assert!(!terminal.succeeded());
}

#[tokio::test]
async fn cleanup_after_nonterminal_join_cannot_authorize_success() {
    let calls = Rc::new(Cell::new(0));
    let probe_calls = calls.clone();
    let terminal = SmokeTerminal::default();
    let result = run_smoke_sequence(
        &terminal,
        move |phase, exchange_id, _| {
            let call = probe_calls.get();
            probe_calls.set(call + 1);
            if call == 0 {
                std::future::ready(Ok(CompletedProbe {
                    message: phase_message(MessageKind::Pending, phase.as_str(), exchange_id),
                    completion: phase_completion(MessageKind::Pending, phase.as_str(), exchange_id),
                    pin: PinnedExchange { exchange_id, phase },
                }))
            } else {
                std::future::ready(Err("window closed before exact terminal completion".into()))
            }
        },
        || std::future::ready(()),
    )
    .await;

    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
    assert!(!terminal.succeeded());
}
