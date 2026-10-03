//! Task 052 §2.3: makes the focus-claim path visible in the
//! release-checks log, the same reasoning as task 036's forwarder/printer
//! for trusted-click (`trusted_click::diagnose`) -- but scoped to
//! `source.focus.*` events, and printed for every release-checks
//! scenario rather than naming the one this task added it for
//! (`keyboard_mode_switch_commits_pending_field`, whose own symptom --
//! a plain timeout waiting for the editor to take focus -- gives no
//! clue by itself which step of the claim/arm/consume sequence it was):
//! a focus event can only ever fire from a scenario that claims editor
//! focus, so it is silent, and costs nothing, in every other one.

use std::sync::Mutex;

/// A log of the `source.focus.*` events one release-checks process (one
/// scenario, task 026's "one process launch per scenario") recorded. No
/// bound: a single scenario's own focus-claim sequence is a handful of
/// events, nothing like trusted-click's many-interaction volume.
#[derive(Debug, Default)]
pub(super) struct FocusTraceLog {
    entries: Mutex<Vec<String>>,
}

impl FocusTraceLog {
    pub(super) const fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn record(&self, event: &str, details: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.push(format!("{event} {details}").trim_end().to_string());
    }

    /// The whole log, one entry per line, for the end of this run. Says
    /// so when nothing was recorded, so silence is never read as "fine".
    pub(super) fn render(&self) -> String {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = format!("focus trace ({} entries):", entries.len());
        if entries.is_empty() {
            out.push_str("\n  none recorded: no source.focus.* trace fired in this run");
        }
        for entry in entries.iter() {
            out.push_str("\n  ");
            out.push_str(entry);
        }
        out
    }
}

static LOG: FocusTraceLog = FocusTraceLog::new();

/// Only the focus-claim path is recorded, and only in a release-checks run.
pub(super) fn wants(event: &str, in_release_checks_run: bool) -> bool {
    in_release_checks_run && event.starts_with("source.focus.")
}

/// Called for every `bridge::trace` event; a no-op, formatting nothing,
/// unless this is a release-checks run and the event is a
/// `source.focus.*` one.
pub(in crate::webview_smoke) fn record_focus_trace(event: &str, details: impl std::fmt::Display) {
    record_into(
        &LOG,
        crate::webview_smoke::in_release_checks_run(),
        event,
        details,
    );
}

/// `record_focus_trace` with the log and the mode given, so each rule is
/// tested without a live run.
fn record_into(
    log: &FocusTraceLog,
    in_release_checks_run: bool,
    event: &str,
    details: impl std::fmt::Display,
) {
    if wants(event, in_release_checks_run) {
        log.record(event, &details.to_string());
    }
}

/// Prints the whole log under its heading. Called at the end of every
/// release-checks run, pass or fail.
pub(super) fn print_final_trace() {
    println!("{}", LOG.render());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Display that must never be formatted.
    struct MustNotFormat;
    impl std::fmt::Display for MustNotFormat {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("details were formatted although nothing was being recorded");
        }
    }

    #[test]
    fn only_focus_events_are_recorded_even_in_the_run_mode() {
        for event in ["source.form_commit.saved", "app_bar.new_file.click"] {
            let log = FocusTraceLog::new();
            record_into(&log, true, event, "x");
            assert!(log.render().contains("(0 entries)"), "{event}");
        }
    }

    #[test]
    fn a_focus_event_reaches_the_log_only_in_the_run_mode() {
        let in_run = FocusTraceLog::new();
        record_into(
            &in_run,
            true,
            "source.focus.interaction.allocate",
            "token=1",
        );
        assert!(
            in_run
                .render()
                .contains("source.focus.interaction.allocate token=1")
        );

        let elsewhere = FocusTraceLog::new();
        record_into(
            &elsewhere,
            false,
            "source.focus.interaction.allocate",
            "token=1",
        );
        assert!(elsewhere.render().contains("(0 entries)"));
    }

    #[test]
    fn a_run_with_nothing_recorded_says_so_plainly() {
        let log = FocusTraceLog::new();
        let text = log.render();
        assert!(text.starts_with("focus trace (0 entries):"), "{text}");
        assert!(text.contains("none recorded"), "{text}");
    }

    #[test]
    fn entries_print_in_order_under_their_heading() {
        let log = FocusTraceLog::new();
        log.record("source.focus.interaction.allocate", "token=1");
        log.record("source.focus.guard.armed", "token=1");
        log.record("source.focus.consumed", "token=1 outcome=accepted");
        let text = log.render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "focus trace (3 entries):");
        assert!(
            lines[1].contains("source.focus.interaction.allocate"),
            "{text}"
        );
        assert!(lines[2].contains("source.focus.guard.armed"), "{text}");
        assert!(lines[3].contains("source.focus.consumed"), "{text}");
    }

    #[test]
    fn outside_the_run_mode_nothing_is_formatted_and_the_shared_log_is_untouched() {
        let before = LOG.render();
        // The unit-test process installs no launch config, so this is not a run.
        record_focus_trace("source.focus.interaction.allocate", MustNotFormat);
        assert_eq!(LOG.render(), before);
    }
}
