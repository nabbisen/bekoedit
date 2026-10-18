//! Task 061: what the lifecycle smoke can say when it fails.
//!
//! The RFC-041 lifecycle step failed once, at `editor_ready_focused`, with a
//! one-line message that could not say which of five conditions was missing or
//! what the focus guard had answered. The page side now reports the conditions
//! (`driver.js`, carried in `DriverResult::diagnosis`). This is the Rust side:
//! the focus path -- `source.focus.interaction.allocate`, `guard.armed`,
//! `guard.rejected`, `guard.timeout`, `command.queued`, and the page's own
//! `source.focus.rejected.guard` and `source.focus.consumed` -- is kept in a
//! lifecycle run and printed at the end of **every** one, pass or fail, so a
//! passing run shows the normal sequence to compare a failing one against.
//! `BEKOEDIT_SOURCE_TRACE` is not involved: CI does not set it.

use super::release_checks::FocusTraceLog;

/// Prefix of every diagnosis line, so a CI log can be searched for them.
const DIAGNOSIS_PREFIX: &str = "bekoedit WebView lifecycle smoke diagnosis:";

static LOG: FocusTraceLog = FocusTraceLog::new();

/// Only the focus path (`source.focus.*`, page and Rust side), and only in a
/// lifecycle run.
pub(super) fn wants(event: &str, in_lifecycle_run: bool) -> bool {
    in_lifecycle_run && event.starts_with("source.focus.")
}

/// Called for every `bridge::trace` event; a no-op, formatting nothing, unless
/// this is a lifecycle run and the event is a `source.focus.*` one.
pub(super) fn record_lifecycle_trace(event: &str, details: impl std::fmt::Display) {
    record_into(&LOG, super::in_lifecycle_run(), event, details);
}

/// `record_lifecycle_trace` with the log and the mode given, so each rule is
/// tested without a live run.
fn record_into(
    log: &FocusTraceLog,
    in_lifecycle_run: bool,
    event: &str,
    details: impl std::fmt::Display,
) {
    if wants(event, in_lifecycle_run) {
        log.record(event, &details.to_string());
    }
}

/// Prints the whole focus log. Called at the end of every lifecycle run.
pub(super) fn print_final_trace() {
    println!("{}", LOG.render());
}

/// The page's diagnosis, one printed line per fact, each led by the prefix.
pub(super) fn diagnosis_lines(page: &[String]) -> Vec<String> {
    page.iter()
        .map(|line| format!("{DIAGNOSIS_PREFIX} {line}"))
        .collect()
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

    const FOCUS_EVENTS: [&str; 7] = [
        "source.focus.interaction.allocate",
        "source.focus.guard.armed",
        "source.focus.guard.rejected",
        "source.focus.guard.timeout",
        "source.focus.command.queued",
        "source.focus.rejected.guard",
        "source.focus.consumed",
    ];

    #[test]
    fn a_lifecycle_run_keeps_every_focus_event_the_task_names() {
        let log = FocusTraceLog::new();
        for event in FOCUS_EVENTS {
            record_into(&log, true, event, "token=1");
        }
        let text = log.render();
        assert!(text.starts_with("focus trace (7 entries):"), "{text}");
        for event in FOCUS_EVENTS {
            assert!(
                text.contains(&format!("{event} token=1")),
                "{event}: {text}"
            );
        }
    }

    #[test]
    fn a_run_of_no_smoke_mode_keeps_nothing() {
        let log = FocusTraceLog::new();
        for event in FOCUS_EVENTS {
            record_into(&log, false, event, "token=1");
        }
        assert!(log.render().contains("(0 entries)"));
    }

    #[test]
    fn only_focus_events_are_kept_even_in_a_lifecycle_run() {
        for event in ["source.form_commit.saved", "app_bar.new_file.click"] {
            let log = FocusTraceLog::new();
            record_into(&log, true, event, "x");
            assert!(log.render().contains("(0 entries)"), "{event}");
        }
    }

    #[test]
    fn outside_a_lifecycle_run_nothing_is_formatted_and_the_shared_log_is_untouched() {
        let before = LOG.render();
        // The unit-test process installs no launch config, so this is not a run.
        record_lifecycle_trace("source.focus.interaction.allocate", MustNotFormat);
        assert_eq!(LOG.render(), before);
    }

    #[test]
    fn every_diagnosis_line_leads_with_the_greppable_prefix() {
        let lines = diagnosis_lines(&["a".into(), "b: c".into()]);
        assert_eq!(
            lines,
            [
                "bekoedit WebView lifecycle smoke diagnosis: a",
                "bekoedit WebView lifecycle smoke diagnosis: b: c",
            ]
        );
    }

    #[test]
    fn no_page_diagnosis_prints_no_lines() {
        assert!(diagnosis_lines(&[]).is_empty());
    }
}
