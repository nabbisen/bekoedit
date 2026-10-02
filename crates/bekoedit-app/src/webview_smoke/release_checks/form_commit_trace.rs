//! Task 049 §2.2: makes the pending-field commit path visible in the
//! release-checks log, the same reasoning as task 036's forwarder/printer
//! for trusted-click (`trusted_click::diagnose`) -- but scoped to
//! `source.form_commit.*` events, since that is the only path this run
//! needs to see, and printed for every release-checks scenario rather
//! than naming the two this task added: a `source.form_commit.*` trace
//! can only ever fire from a Form Mode scenario, so it is silent, and
//! this costs nothing, in every other one.

use std::sync::Mutex;

/// A log of the `source.form_commit.*` events one release-checks process
/// (one scenario, task 026's "one process launch per scenario") recorded.
/// No bound: the whole exchange this run can ever produce is a handful of
/// entries per command, nothing like trusted-click's many-interaction
/// volume.
#[derive(Debug, Default)]
pub(super) struct FormCommitLog {
    entries: Mutex<Vec<String>>,
}

impl FormCommitLog {
    pub(super) const fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn record(&self, event: &str, details: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.push(format!("{event} {details}").trim_end().to_string());
    }

    /// The whole log, one entry per line, for the end of this run. Says so
    /// when nothing was recorded, so silence is never read as "fine".
    pub(super) fn render(&self) -> String {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = format!("form-commit trace ({} entries):", entries.len());
        if entries.is_empty() {
            out.push_str("\n  none recorded: no source.form_commit.* trace fired in this run");
        }
        for entry in entries.iter() {
            out.push_str("\n  ");
            out.push_str(entry);
        }
        out
    }
}

static LOG: FormCommitLog = FormCommitLog::new();

/// Only the form-commit path is recorded, and only in a release-checks run.
pub(super) fn wants(event: &str, in_release_checks_run: bool) -> bool {
    in_release_checks_run && event.starts_with("source.form_commit.")
}

/// Called for every `bridge::trace` event; a no-op, formatting nothing,
/// unless this is a release-checks run and the event is a
/// `source.form_commit.*` one.
pub(in crate::webview_smoke) fn record_form_commit_trace(
    event: &str,
    details: impl std::fmt::Display,
) {
    record_into(
        &LOG,
        crate::webview_smoke::in_release_checks_run(),
        event,
        details,
    );
}

/// `record_form_commit_trace` with the log and the mode given, so each
/// rule is tested without a live run.
fn record_into(
    log: &FormCommitLog,
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
    fn only_form_commit_events_are_recorded_even_in_the_run_mode() {
        for event in [
            "source.focus.interaction.allocate",
            "app_bar.new_file.click",
        ] {
            let log = FormCommitLog::new();
            record_into(&log, true, event, "x");
            assert!(log.render().contains("(0 entries)"), "{event}");
        }
    }

    #[test]
    fn a_form_commit_event_reaches_the_log_only_in_the_run_mode() {
        let in_run = FormCommitLog::new();
        record_into(
            &in_run,
            true,
            "source.form_commit.read",
            "id=fb-1-2 value_len=3",
        );
        assert!(
            in_run
                .render()
                .contains("source.form_commit.read id=fb-1-2 value_len=3")
        );

        let elsewhere = FormCommitLog::new();
        record_into(
            &elsewhere,
            false,
            "source.form_commit.read",
            "id=fb-1-2 value_len=3",
        );
        assert!(elsewhere.render().contains("(0 entries)"));
    }

    #[test]
    fn a_run_with_nothing_recorded_says_so_plainly() {
        let log = FormCommitLog::new();
        let text = log.render();
        assert!(text.starts_with("form-commit trace (0 entries):"), "{text}");
        assert!(text.contains("none recorded"), "{text}");
    }

    #[test]
    fn entries_print_in_order_under_their_heading() {
        let log = FormCommitLog::new();
        log.record(
            "source.form_commit.read",
            "id=fb-1-2 value_len=7 composing=false",
        );
        log.record("source.form_commit.committed", "fb-1-2");
        let text = log.render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "form-commit trace (2 entries):");
        assert!(lines[1].contains("source.form_commit.read"), "{text}");
        assert!(lines[2].contains("source.form_commit.committed"), "{text}");
    }

    #[test]
    fn outside_the_run_mode_nothing_is_formatted_and_the_shared_log_is_untouched() {
        let before = LOG.render();
        // The unit-test process installs no launch config, so this is not a run.
        record_form_commit_trace("source.form_commit.read", MustNotFormat);
        assert_eq!(LOG.render(), before);
    }
}
