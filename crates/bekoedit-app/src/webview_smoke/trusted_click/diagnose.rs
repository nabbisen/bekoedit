//! Task 029: what the app itself knew when a trusted-click phase failed.
//!
//! The `tree_row_focus` timeout was seen three times (runs `35810114012`,
//! `36008866139`, `36077020607`) with a one-line message that could not say
//! which of these happened: the click never landed; it reached the row but no
//! document opened; the document opened but the editor never became ready; the
//! editor was ready but the focus claim was refused; focus landed and was lost.
//! The page-side half (focus, active element, tree rows, editor state, and the
//! first change with its time) is in `trusted_click_driver.js`. This is the
//! Rust-side half: which document is open, the mode, the source-sync lifecycle
//! state, and what the focus guard answered.
//!
//! The guard's answers reach Rust as `SourceEditorEvent::Trace` events and were
//! only ever printed under `BEKOEDIT_SOURCE_TRACE`. `record_source_trace` keeps
//! the most recent ones, and only in a trusted-click run.
//!
//! Task 036: the Rust side's own focus traces (`bridge::trace`:
//! `source.focus.interaction.allocate`, `guard.armed`, `guard.rejected`,
//! `guard.timeout`, `command.queued`) enter the same log, so it holds the whole
//! focus path and not only the half the page reports. And the log is printed at
//! the end of **every** run, pass or fail, so a passing run shows the normal
//! sequence to compare a failing one against.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::source_sync::SourceSyncState;
use crate::source_sync::lifecycle::LifecycleState;

use super::super::transport::DriverResult;

/// How many entries the log holds. One whole run makes about ten interactions,
/// each with a click note and up to five traces, so 24 (task 029's bound) would
/// drop the early ones, and `tree_row_focus` is the second phase. The failure
/// message still shows only the last `IN_MESSAGE`, as task 029 shipped it.
const CAPACITY: usize = 256;
const IN_MESSAGE: usize = 24;

/// Only the focus path is recorded (`source.focus.*`, page and Rust side), and
/// only in a trusted-click run.
pub(super) fn wants(event: &str, in_trusted_click_run: bool) -> bool {
    in_trusted_click_run && event.starts_with("source.focus.")
}

#[derive(Debug, Default)]
struct Entries {
    lines: VecDeque<String>,
    /// How many older entries `CAPACITY` pushed out.
    dropped: usize,
}

/// A bounded log of timestamped notes: focus traces the app received or made,
/// and the moments this run clicked.
#[derive(Debug, Default)]
pub(super) struct TraceLog {
    entries: Mutex<Entries>,
}

impl TraceLog {
    pub(super) const fn new() -> Self {
        Self {
            entries: Mutex::new(Entries {
                lines: VecDeque::new(),
                dropped: 0,
            }),
        }
    }

    pub(super) fn record(&self, at_ms: u128, event: &str, details: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.lines.len() == CAPACITY {
            entries.lines.pop_front();
            entries.dropped += 1;
        }
        entries.lines.push_back(
            format!("+{at_ms}ms {event} {details}")
                .trim_end()
                .to_string(),
        );
    }

    /// The recent focus claim, if the app recorded one, else a plain statement
    /// that nothing did -- never an empty string that could be misread as
    /// "fine". Only the last `IN_MESSAGE` entries, as in a failure message.
    pub(super) fn describe(&self) -> String {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let recent: Vec<&String> = entries
            .lines
            .iter()
            .skip(entries.lines.len().saturating_sub(IN_MESSAGE))
            .collect();
        let all = recent
            .iter()
            .map(|entry| entry.as_str())
            .collect::<Vec<_>>()
            .join(" ; ");
        if recent.iter().any(|entry| entry.contains(" source.focus.")) {
            format!("recent notes (oldest first): {all}")
        } else {
            format!(
                "no source.focus.* trace was recorded, so the focus claim was not attempted or \
                 nothing reported it; recent notes: [{all}]"
            )
        }
    }

    /// The whole log, one entry per line, for the end of every run. It says so
    /// when the bound was reached (so a truncated log is never read as
    /// complete) and when nothing was recorded (so silence is never read as
    /// "fine").
    pub(super) fn render_final(&self) -> String {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let count = entries.lines.len();
        let mut out = format!("trusted-click focus trace ({count} entries):");
        if count == 0 {
            out.push_str(
                "\n  none recorded: no run note and no source.focus.* trace, from the page or \
                 from Rust",
            );
        }
        for line in &entries.lines {
            out.push_str("\n  ");
            out.push_str(line);
        }
        if entries.dropped > 0 {
            out.push_str(&format!(
                "\n  BOUND REACHED: the log holds at most {CAPACITY} entries and {} older \
                 entries were dropped, so this is NOT the whole run",
                entries.dropped
            ));
        } else if !entries
            .lines
            .iter()
            .any(|line| line.contains(" source.focus."))
        {
            out.push_str("\n  no source.focus.* trace was recorded in this run");
        }
        out
    }
}

static LOG: TraceLog = TraceLog::new();
static CLOCK: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u128 {
    CLOCK.get_or_init(Instant::now).elapsed().as_millis()
}

/// Starts the clock the notes are stamped against.
pub(super) fn start_clock() {
    let _ = now_ms();
}

/// A moment this run caused, for example "tree_row_focus: click sent".
pub(super) fn note(text: &str) {
    LOG.record(now_ms(), "run:", text);
}

/// Called for every `bridge::trace` event; a no-op, formatting nothing, unless
/// this is a trusted-click run and the event is a `source.focus.*` one.
pub(in crate::webview_smoke) fn record_source_trace(event: &str, details: impl std::fmt::Display) {
    record_into(
        &LOG,
        crate::webview_smoke::in_trusted_click_run(),
        now_ms(),
        event,
        details,
    );
}

/// `record_source_trace` with the log, the mode and the time given, so each
/// rule is tested without a live run.
fn record_into(
    log: &TraceLog,
    in_trusted_click_run: bool,
    at_ms: u128,
    event: &str,
    details: impl std::fmt::Display,
) {
    if wants(event, in_trusted_click_run) {
        log.record(at_ms, event, &details.to_string());
    }
}

/// Prints the whole log under its heading. Called at the end of every run.
pub(super) fn print_final_trace() {
    println!("{}", LOG.render_final());
}

pub(super) const fn lifecycle_name(state: &LifecycleState) -> &'static str {
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

/// The app-side facts, as one line. Pure, so each field is tested.
pub(super) fn describe_app(document: Option<&str>, mode: EditorMode, lifecycle: &str) -> String {
    format!(
        "app: open document={} mode={mode:?} source-sync lifecycle={lifecycle}",
        document.unwrap_or("none")
    )
}

/// What the app knew, read from the live signals; `try_peek`, so a borrow held
/// elsewhere is reported instead of panicking.
pub(super) fn describer(
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    sync: Signal<SourceSyncState>,
) -> impl FnMut() -> String {
    move || {
        let (Ok(state), Ok(sync)) = (state.try_peek(), sync.try_peek()) else {
            return "app state was borrowed elsewhere".to_string();
        };
        let document = state
            .session
            .as_ref()
            .and_then(|session| session.path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        format!(
            "{} | {}",
            describe_app(
                document.as_deref(),
                *mode.peek(),
                lifecycle_name(&sync.lifecycle.state)
            ),
            LOG.describe()
        )
    }
}

/// Appends `context` to a driver failure's own reason; a success is untouched.
pub(super) fn enrich_failure(result: &mut DriverResult, context: &str) {
    if result.ok {
        return;
    }
    let reason = result.error.as_deref().unwrap_or("unknown error");
    result.error = Some(format!("{reason} | {context}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(error: &str) -> DriverResult {
        DriverResult {
            ok: false,
            stage: "tree_row_trusted_click_focused_editor".into(),
            marker: "m".into(),
            milestones: Vec::new(),
            error_toast_seen: false,
            error: Some(error.into()),
        }
    }

    #[test]
    fn the_app_side_line_names_the_document_the_mode_and_the_lifecycle() {
        let line = describe_app(Some("child.md"), EditorMode::Text, "Ready");
        assert_eq!(
            line,
            "app: open document=child.md mode=Text source-sync lifecycle=Ready"
        );
        let none = describe_app(None, EditorMode::Form, "Mounting");
        assert!(
            none.contains("open document=none") && none.contains("mode=Form"),
            "{none}"
        );
        assert!(none.contains("lifecycle=Mounting"), "{none}");
    }

    #[test]
    fn a_refused_focus_claim_shows_what_the_guard_answered() {
        let log = TraceLog::new();
        log.record(10, "run:", "tree_row_focus: click sent");
        log.record(
            240,
            "source.focus.rejected.guard",
            "token=7 reason=activeElementIneligible outcome=rejected active=other",
        );
        let text = log.describe();
        assert!(text.starts_with("recent notes (oldest first): "), "{text}");
        assert!(
            text.contains(
                "+240ms source.focus.rejected.guard token=7 reason=activeElementIneligible"
            ),
            "{text}"
        );
        assert!(
            text.contains("+10ms run: tree_row_focus: click sent"),
            "{text}"
        );
    }

    #[test]
    fn no_recorded_claim_is_said_plainly_never_left_blank() {
        let log = TraceLog::new();
        assert!(
            log.describe()
                .contains("no source.focus.* trace was recorded")
        );
        log.record(5, "run:", "tree_row_focus: click sent");
        let text = log.describe();
        assert!(
            text.contains("no source.focus.* trace was recorded"),
            "{text}"
        );
        assert!(text.contains("click sent"), "{text}");
    }

    #[test]
    fn the_log_keeps_only_the_most_recent_notes() {
        let log = TraceLog::new();
        for i in 0..(IN_MESSAGE + 5) {
            log.record(i as u128, "source.focus.consumed", &format!("n={i}"));
        }
        let text = log.describe();
        assert!(!text.contains("n=4 ") && !text.ends_with("n=4"), "{text}");
        assert!(text.contains(&format!("n={}", IN_MESSAGE + 4)), "{text}");
        assert_eq!(text.matches("source.focus.consumed").count(), IN_MESSAGE);
    }

    #[test]
    fn a_failure_keeps_its_own_reason_and_gains_the_context() {
        let mut result =
            failed("Error: timed out at tree_row_trusted_click_focused_editor. At the timeout: x");
        enrich_failure(&mut result, "app: open document=none");
        let error = result.error.unwrap();
        assert!(
            error.starts_with("Error: timed out at tree_row_trusted_click_focused_editor."),
            "{error}"
        );
        assert!(error.ends_with("| app: open document=none"), "{error}");
    }

    #[test]
    fn a_success_is_never_touched() {
        let mut result = failed("x");
        result.ok = true;
        result.error = None;
        enrich_failure(&mut result, "context");
        assert_eq!(result.error, None);
    }

    /// Display that must never be formatted.
    struct MustNotFormat;
    impl std::fmt::Display for MustNotFormat {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("details were formatted although nothing was being recorded");
        }
    }

    const RUST_SIDE: [&str; 5] = [
        "source.focus.interaction.allocate",
        "source.focus.guard.armed",
        "source.focus.guard.rejected",
        "source.focus.guard.timeout",
        "source.focus.command.queued",
    ];
    const PAGE_SIDE: [&str; 3] = [
        "source.focus.consumed",
        "source.focus.rejected.identity",
        "source.focus.rejected.guard",
    ];

    #[test]
    fn a_rust_side_focus_trace_reaches_the_log_only_in_the_run_mode() {
        for event in RUST_SIDE.iter().chain(PAGE_SIDE.iter()) {
            let in_run = TraceLog::new();
            record_into(&in_run, true, 5, event, "token=1");
            assert!(
                in_run.describe().contains(event),
                "{event} not recorded in the run"
            );

            let elsewhere = TraceLog::new();
            record_into(&elsewhere, false, 5, event, "token=1");
            assert!(
                elsewhere.render_final().contains("(0 entries)"),
                "{event} was recorded outside the run mode"
            );
        }
    }

    #[test]
    fn only_focus_events_are_recorded_even_in_the_run_mode() {
        for event in [
            "source.controller.stale",
            "app_bar.new_file.click",
            "source.relay.restart",
        ] {
            let log = TraceLog::new();
            record_into(&log, true, 1, event, "x");
            assert!(log.render_final().contains("(0 entries)"), "{event}");
        }
    }

    #[test]
    fn outside_the_run_mode_nothing_is_formatted_and_the_shared_log_is_untouched() {
        let before = LOG.render_final();
        // The unit-test process installs no launch config, so this is not a run.
        record_source_trace("source.focus.interaction.allocate", MustNotFormat);
        assert_eq!(LOG.render_final(), before);
    }

    #[test]
    fn the_printed_log_says_so_when_it_hits_the_bound() {
        let log = TraceLog::new();
        for i in 0..CAPACITY {
            log.record(i as u128, "source.focus.consumed", &format!("n={i}"));
        }
        let full = log.render_final();
        assert!(
            !full.contains("BOUND REACHED"),
            "exactly full is not truncated"
        );
        assert!(full.contains(&format!("({CAPACITY} entries)")), "{full}");

        log.record(9999, "source.focus.consumed", "n=last");
        log.record(10000, "source.focus.consumed", "n=last2");
        let over = log.render_final();
        assert!(over.contains("BOUND REACHED"), "{over}");
        assert!(over.contains("2 older entries were dropped"), "{over}");
        assert!(over.contains("NOT the whole run"), "{over}");
        assert!(
            !over.contains("n=0\n") && over.contains("n=last2"),
            "the oldest went, the newest stayed"
        );
    }

    #[test]
    fn a_run_with_no_trace_at_all_prints_that_plainly() {
        let empty = TraceLog::new().render_final();
        assert!(
            empty.starts_with("trusted-click focus trace (0 entries):"),
            "{empty}"
        );
        assert!(empty.contains("none recorded"), "{empty}");

        let notes_only = TraceLog::new();
        notes_only.record(3, "run:", "tree_row_focus: click sent");
        let text = notes_only.render_final();
        assert!(
            text.contains("(1 entries)") && text.contains("click sent"),
            "{text}"
        );
        assert!(
            text.contains("no source.focus.* trace was recorded in this run"),
            "{text}"
        );
    }

    #[test]
    fn a_normal_sequence_prints_in_order_under_its_heading() {
        let log = TraceLog::new();
        log.record(625, "run:", "tree_row_focus: click sent");
        log.record(640, "source.focus.interaction.allocate", "token=3");
        log.record(660, "source.focus.guard.armed", "token=3");
        log.record(670, "source.focus.command.queued", "Queued");
        log.record(700, "source.focus.consumed", "token=3");
        let text = log.render_final();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "trusted-click focus trace (5 entries):");
        assert!(
            lines[1].contains("click sent") && lines[2].contains("allocate"),
            "{text}"
        );
        assert!(
            lines[3].contains("armed") && lines[4].contains("queued"),
            "{text}"
        );
        assert!(lines[5].contains("consumed"), "{text}");
        assert_eq!(
            lines.len(),
            6,
            "no bound note and no 'no trace' note: {text}"
        );
    }

    /// The wiring, by source shape (the forwarding needs a live run to observe):
    /// `bridge::trace` forwards, `host.rs` no longer records the same event a
    /// second time, and the five Rust-side traces the task names are made in
    /// `focus.rs` through `bridge::trace`.
    #[test]
    fn the_forwarding_is_wired_once_and_covers_the_rust_side_traces() {
        let bridge = include_str!("../../bridge.rs");
        let trace_fn = bridge
            .split("pub fn trace(")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .expect("bridge::trace");
        assert!(
            trace_fn.contains("record_source_trace(event, &details)"),
            "{trace_fn}"
        );

        let host = include_str!("../../source_sync/host.rs");
        assert!(
            !host.contains("record_source_trace("),
            "host.rs would record page traces twice"
        );

        let focus = include_str!("../../source_sync/focus.rs");
        for name in RUST_SIDE {
            assert!(
                focus.contains(&format!("\"{name}\"")),
                "{name} is not traced in focus.rs"
            );
        }
    }
}
