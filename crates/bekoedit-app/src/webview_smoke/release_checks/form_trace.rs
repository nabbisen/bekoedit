//! Task 056 §2.2: makes a refused Form edit visible in the
//! release-checks log, the same reasoning and shape as task 052's
//! `focus_trace` forwarder/printer -- but scoped to `form.*` events, and
//! printed for every release-checks scenario rather than naming the one
//! that found the need for it (`table_row_insert_and_delete`, whose own
//! symptom -- a real click, no effect -- gave no clue by itself whether
//! the edit was even attempted, let alone refused).

use std::sync::Mutex;

/// A log of the `form.*` events one release-checks process (one
/// scenario, task 026's "one process launch per scenario") recorded. No
/// bound: a Form Mode scenario dispatches a handful of edits at most.
#[derive(Debug, Default)]
pub(super) struct FormTraceLog {
    entries: Mutex<Vec<String>>,
}

impl FormTraceLog {
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
        let mut out = format!("form trace ({} entries):", entries.len());
        if entries.is_empty() {
            out.push_str("\n  none recorded: no form.* trace fired in this run");
        }
        for entry in entries.iter() {
            out.push_str("\n  ");
            out.push_str(entry);
        }
        out
    }
}

static LOG: FormTraceLog = FormTraceLog::new();

/// Only Form Mode's own trace events are recorded, and only in a
/// release-checks run.
pub(super) fn wants(event: &str, in_release_checks_run: bool) -> bool {
    in_release_checks_run && event.starts_with("form.")
}

/// Called for every `bridge::trace` event; a no-op, formatting nothing,
/// unless this is a release-checks run and the event is a `form.*` one.
pub(in crate::webview_smoke) fn record_form_trace(event: &str, details: impl std::fmt::Display) {
    record_into(
        &LOG,
        crate::webview_smoke::in_release_checks_run(),
        event,
        details,
    );
}

/// `record_form_trace` with the log and the mode given, so each rule is
/// tested without a live run.
fn record_into(
    log: &FormTraceLog,
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
    fn only_form_events_are_recorded_even_in_the_run_mode() {
        for event in [
            "source.focus.interaction.allocate",
            "app_bar.new_file.click",
        ] {
            let log = FormTraceLog::new();
            record_into(&log, true, event, "x");
            assert!(log.render().contains("(0 entries)"), "{event}");
        }
    }

    #[test]
    fn a_form_event_reaches_the_log_only_in_the_run_mode() {
        let in_run = FormTraceLog::new();
        record_into(&in_run, true, "form.edit_refused", "kind=block_not_found");
        assert!(
            in_run
                .render()
                .contains("form.edit_refused kind=block_not_found")
        );

        let elsewhere = FormTraceLog::new();
        record_into(
            &elsewhere,
            false,
            "form.edit_refused",
            "kind=block_not_found",
        );
        assert!(elsewhere.render().contains("(0 entries)"));
    }

    #[test]
    fn a_run_with_nothing_recorded_says_so_plainly() {
        let log = FormTraceLog::new();
        let text = log.render();
        assert!(text.starts_with("form trace (0 entries):"), "{text}");
        assert!(text.contains("none recorded"), "{text}");
    }

    #[test]
    fn outside_the_run_mode_nothing_is_formatted_and_the_shared_log_is_untouched() {
        let before = LOG.render();
        // The unit-test process installs no launch config, so this is not a run.
        record_form_trace("form.edit_refused", MustNotFormat);
        assert_eq!(LOG.render(), before);
    }
}
