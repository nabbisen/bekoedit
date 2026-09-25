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

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::source_sync::SourceSyncState;
use crate::source_sync::lifecycle::LifecycleState;

use super::super::transport::DriverResult;

const KEEP: usize = 24;

/// A bounded log of timestamped notes: page traces the app received, and the
/// moments this run clicked.
#[derive(Debug, Default)]
pub(super) struct TraceLog {
    entries: Mutex<VecDeque<String>>,
}

impl TraceLog {
    pub(super) const fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
        }
    }

    pub(super) fn record(&self, at_ms: u128, event: &str, details: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.len() == KEEP {
            entries.pop_front();
        }
        entries.push_back(
            format!("+{at_ms}ms {event} {details}")
                .trim_end()
                .to_string(),
        );
    }

    /// The focus claim, if the app recorded one, else a plain statement that
    /// nothing did -- never an empty string that could be misread as "fine".
    pub(super) fn describe(&self) -> String {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let all = entries.iter().cloned().collect::<Vec<_>>().join(" ; ");
        if entries.iter().any(|entry| entry.contains(" source.focus.")) {
            format!("recent notes (oldest first): {all}")
        } else {
            format!(
                "no source.focus.* trace was recorded, so the focus claim was not attempted or \
                 nothing reported it; recent notes: [{all}]"
            )
        }
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

/// Called for every page trace event; a no-op unless this is a trusted-click run.
pub(in crate::webview_smoke) fn record_source_trace(event: &str, details: &str) {
    if crate::webview_smoke::launch_config()
        .trusted_click
        .is_some()
    {
        LOG.record(now_ms(), event, details);
    }
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
        for i in 0..(KEEP + 5) {
            log.record(i as u128, "source.focus.consumed", &format!("n={i}"));
        }
        let text = log.describe();
        assert!(!text.contains("n=4 ") && !text.ends_with("n=4"), "{text}");
        assert!(text.contains(&format!("n={}", KEEP + 4)), "{text}");
        assert_eq!(text.matches("source.focus.consumed").count(), KEEP);
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
}
