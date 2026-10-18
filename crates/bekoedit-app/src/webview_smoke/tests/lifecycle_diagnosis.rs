//! Task 061: the lifecycle smoke says why it failed. The page's diagnosis
//! must survive the trip to the terminal, and the run must print it, and the
//! focus trace, where a CI log shows them.

use super::*;

fn editor_timeout() -> DriverResult {
    DriverResult {
        ok: false,
        stage: "editor_ready_focused".into(),
        marker: MARKER.into(),
        milestones: vec!["observer_installed".into(), "new_clicked".into()],
        error_toast_seen: false,
        error: Some("Error: timed out at editor_ready_focused".into()),
        diagnosis: vec![
            "editor conditions: window.__bk._view=true view.dom.isConnected=true \
             view.hasFocus=false textHost=true hostHasNoStatusMarker=true"
                .into(),
            "document.hasFocus()=true; activeElement=body".into(),
        ],
    }
}

async fn run_to(
    terminal: &SmokeTerminal,
    editor_result: DriverResult,
) -> Result<DriverResult, String> {
    run_smoke_sequence(
        terminal,
        move |phase, exchange_id, release| {
            let (kind, milestone, result) = match phase {
                SmokePhase::Launch => (MessageKind::Progress, Some("new_clicked"), None),
                SmokePhase::Editor => (MessageKind::Terminal, None, Some(editor_result.clone())),
                SmokePhase::Preview => unreachable!("the editor phase is terminal here"),
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
        || std::future::ready(()),
    )
    .await
}

#[tokio::test]
async fn an_editor_timeout_reaches_the_terminal_with_the_pages_diagnosis() {
    let terminal = SmokeTerminal::default();
    let result = run_to(&terminal, editor_timeout()).await;

    assert_eq!(
        result.unwrap_err(),
        "driver failed at editor_ready_focused: Error: timed out at editor_ready_focused"
    );
    assert!(!terminal.succeeded());
    assert_eq!(terminal.diagnosis(), editor_timeout().diagnosis);
}

#[tokio::test]
async fn a_run_with_no_failure_keeps_no_diagnosis() {
    let terminal = SmokeTerminal::default();
    let mut success = successful_result();
    // Even a stray diagnosis on a success is not kept: only a failure has one.
    success.diagnosis = vec!["stray".into()];

    terminal.accept(&success).unwrap();

    assert!(terminal.succeeded());
    assert!(terminal.diagnosis().is_empty());
}

#[test]
fn a_result_without_a_diagnosis_still_parses_and_one_with_it_carries_it() {
    let bare = r#"{"ok":false,"stage":"s","marker":"m","milestones":[],
        "errorToastSeen":false,"error":"e"}"#;
    let with = r#"{"ok":false,"stage":"s","marker":"m","milestones":[],
        "errorToastSeen":false,"error":"e","diagnosis":["one","two"]}"#;

    assert!(
        serde_json::from_str::<DriverResult>(bare)
            .unwrap()
            .diagnosis
            .is_empty()
    );
    assert_eq!(
        serde_json::from_str::<DriverResult>(with)
            .unwrap()
            .diagnosis,
        ["one", "two"]
    );
}

/// The wiring in `webview_smoke.rs` itself. A structural check, in a file of
/// its own: `!source.contains(..)` against the file that holds the needle
/// would test nothing.
mod wiring {
    const SOURCE: &str = include_str!("../../webview_smoke.rs");

    #[test]
    fn the_shared_trace_forwarder_feeds_the_lifecycle_log() {
        let start = SOURCE.find("pub fn record_source_trace").unwrap();
        let body = &SOURCE[start..start + SOURCE[start..].find("\n}\n").unwrap()];
        assert!(
            body.contains("lifecycle_trace::record_lifecycle_trace(event, &details)"),
            "{body}"
        );
    }

    #[test]
    fn a_lifecycle_run_is_the_webview_smoke_mode() {
        let start = SOURCE.find("fn in_lifecycle_run").unwrap();
        let body = &SOURCE[start..start + SOURCE[start..].find("\n}\n").unwrap()];
        assert!(body.contains("config.webview_smoke"), "{body}");
    }

    #[test]
    fn a_failure_prints_the_diagnosis_after_the_failed_line_and_every_run_the_trace() {
        let failed = SOURCE
            .find(r#"eprintln!("bekoedit WebView lifecycle smoke FAILED: {error}")"#)
            .unwrap();
        let diagnosis = SOURCE
            .find("lifecycle_trace::diagnosis_lines(&terminal.diagnosis())")
            .unwrap();
        let trace = SOURCE
            .find("lifecycle_trace::print_final_trace();")
            .unwrap();
        let close = SOURCE.find("desktop.close();").unwrap();
        assert!(
            failed < diagnosis,
            "the diagnosis must follow the FAILED line"
        );
        assert!(diagnosis < trace, "the trace follows the diagnosis");
        assert!(trace < close, "the trace prints before the window closes");
        // After the match, so a passing run prints it too.
        let after_match = SOURCE[failed..].find("\n            }\n").unwrap() + failed;
        assert!(
            trace > after_match,
            "the trace must not sit inside the Err arm"
        );
    }
}
