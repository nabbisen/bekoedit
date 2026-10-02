//! Task 048 §2.1/§2.4, scenario `save_pending_field`: separates D1 from
//! D2 -- with **no toolbar click at all**, does Ctrl+S alone commit a
//! Form Mode field's pending (typed, never blurred) text?
//!
//! `toolbar_probe` alone could not tell D1 (Ctrl+S misses pending text)
//! apart from D2 (the toolbar resolves against stale text): its one
//! scenario exercises both at once. This scenario isolates D1.
//!
//! Task 050 §2.1: this used to set the field's value from script, which
//! task 049's first real run showed is not a valid model here either --
//! its own trace came back `source.form_commit.read null (nothing in
//! .form-mode has focus)`, meaning *something* moved focus away from the
//! field between the script's own `el.focus()` and the queued commit's
//! read, with no click anywhere in this scenario to explain it. A
//! script-set value never sets the HTML "dirty value" flag a
//! blur-triggered `change` needs, so whatever moved focus would have lost
//! the pending text even if D1's own commit worked perfectly -- the
//! scenario was not testing what it claimed to, the same defect task 049
//! already found and fixed in the mode-switch scenario. Real keystrokes
//! (`xdotool type`) fix that: a blur now commits the text through the
//! field's own `onchange`, same as D1's queued commit would, so this
//! scenario can no longer confuse "the test's model is wrong" with "D1
//! is wrong".
//!
//! Task 050 §2.4: also reports what has focus right after the click into
//! the field and again just before Ctrl+S, so a difference (a focus
//! thief) is visible without being asserted into a failure -- it would
//! be a usability defect, not data loss, since a real user's typing
//! commits through blur regardless.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::source_sync::form_commit::{ActiveElementDescription, describe_active_element_js};
use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "save_pending_field";

/// What the field is expected to hold once the real keystrokes below
/// land: the seeded `abc`, plus the typed ` def`.
const TYPED_VALUE: &str = "abc def";

/// What a save settles to within, past the autosave/Ctrl+S write -- the same
/// margin `save.rs`'s own poll uses.
const SAVE_DEADLINE: Duration = Duration::from_secs(10);

/// The fixed, correct outcome: the pending text is committed and saved,
/// verbatim, with no toggle involved at all (task 048 D1).
const EXPECTED_SAVED_TEXT: &str = "abc def\n";

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    desktop: &DesktopContext,
) -> Result<Vec<String>, String> {
    let expectation = &terminal.expectation;
    let file = expectation
        .file
        .as_ref()
        .ok_or_else(|| format!("{NAME}: no file was seeded"))?;

    wait_until(
        NAME,
        "the workspace tree to show the seeded file",
        || async { Ok(dom::snapshot().await?.tree_rows >= 1) },
    )
    .await?;
    activate_window(desktop);
    click_via_xtest(desktop, ".tree-row.tree-file", Some(SAVE_FILE), 0).await?;
    wait_until(NAME, "Form Mode to show the seeded paragraph", || async {
        dom::form_paragraph_present().await
    })
    .await?;

    // A real click into the field, then real keystrokes -- never the
    // field's `.value` set from script (task 050 §2.1's fix: see the
    // module doc comment for why that was never a valid model here).
    click_via_xtest(desktop, ".form-mode .paragraph-input", None, 0).await?;
    let focus_after_click: ActiveElementDescription =
        dom::run_script(&describe_active_element_js()).await?;

    run_xdotool(&["key", "--clearmodifiers", "End"]).await?;
    run_xdotool(&["type", "--clearmodifiers", " def"]).await?;
    wait_until(
        NAME,
        "the paragraph field to hold the typed text",
        || async { dom::paragraph_field_value_is(TYPED_VALUE).await },
    )
    .await?;

    // Task 050 §2.4: what has focus right before the real act under
    // test, reported alongside the post-click reading below -- a
    // difference names a focus thief without failing the scenario over
    // it (§2.4 is a report, not an assertion).
    let focus_before_save: ActiveElementDescription =
        dom::run_script(&describe_active_element_js()).await?;

    // The real act under test: a real Ctrl+S, with the field still
    // focused -- no toolbar click, no blur, nothing else in play.
    let before = std::fs::read(file)
        .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let started = tokio::time::Instant::now();
    let saved = loop {
        let saved = std::fs::read(file)
            .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
        if saved != before {
            tokio::time::sleep(Duration::from_millis(300)).await;
            break std::fs::read(file)
                .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
        }
        if started.elapsed() >= SAVE_DEADLINE {
            break saved;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let saved_text = String::from_utf8_lossy(&saved).into_owned();
    if saved_text != EXPECTED_SAVED_TEXT {
        return Err(format!(
            "{NAME}: expected the saved bytes to be {EXPECTED_SAVED_TEXT:?} (the pending \
             text committed by Ctrl+S alone), got {saved_text:?}; focus right after the \
             click into the field was {}; right before Ctrl+S it was {}",
            focus_after_click.trace_line(),
            focus_before_save.trace_line()
        ));
    }

    let focus_report = if focus_after_click == focus_before_save {
        format!(
            "focus thief: none -- unchanged from the click to Ctrl+S ({})",
            focus_after_click.trace_line()
        )
    } else {
        format!(
            "focus thief: something moved focus between the click and Ctrl+S -- right \
             after the click, {}; right before Ctrl+S, {}",
            focus_after_click.trace_line(),
            focus_before_save.trace_line()
        )
    };

    Ok(vec![
        format!("file on disk after Ctrl+S: {saved_text:?}"),
        focus_report,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seeded paragraph is `abc\n` (`seed.rs`'s `original_paragraph_note`);
    /// `End` then ` def` must land exactly on `TYPED_VALUE`, and the saved
    /// bytes on `TYPED_VALUE` plus the file's own trailing newline --
    /// independent of a live WebView, since a wrong constant here would
    /// make the whole scenario probe the wrong text without ever failing.
    #[test]
    fn the_seeded_text_plus_the_typed_suffix_is_the_expected_value() {
        assert_eq!(format!("{}{}", "abc", " def"), TYPED_VALUE);
        assert_eq!(format!("{TYPED_VALUE}\n"), EXPECTED_SAVED_TEXT);
    }
}
