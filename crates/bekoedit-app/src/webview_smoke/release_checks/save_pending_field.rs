//! Task 048 §2.1/§2.4, scenario `save_pending_field`: separates D1 from
//! D2 -- with **no toolbar click at all**, does Ctrl+S alone commit a
//! Form Mode field's pending (typed, never blurred) text?
//!
//! `toolbar_probe` alone could not tell D1 (Ctrl+S misses pending text)
//! apart from D2 (the toolbar resolves against stale text): its one
//! scenario exercises both at once. This scenario isolates D1: set the
//! field's value from script (no `change` event, same as real typing),
//! press a real Ctrl+S with the field still focused, and assert the saved
//! bytes. Before task 048 D1's fix (`form_commit::commit_pending_form_field`,
//! ordered before every command that saves or leaves the document's
//! current state), this would have saved the seeded `abc\n`, unchanged --
//! `shortcuts.js`'s `keydown` capture never touches DOM focus, so the
//! field's own `onchange` (which fires only on blur) never ran.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "save_pending_field";

/// Sets the paragraph field's value, never leaving it (no `change` event),
/// exactly the uncommitted state a real keystroke leaves. No selection is
/// set: this scenario presses Ctrl+S alone, never the toolbar.
const SET_FIELD_JS: &str = "const el = document.querySelector('.form-mode .paragraph-input'); \
     if (!el) return false; \
     el.focus(); \
     el.value = 'abc def'; \
     return document.activeElement === el && el.value === 'abc def';";

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

    let set_up: bool = dom::run_script(SET_FIELD_JS).await?;
    if !set_up {
        return Err(format!(
            "{NAME}: could not set the paragraph field's pending value"
        ));
    }

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
             text committed by Ctrl+S alone), got {saved_text:?}"
        ));
    }

    Ok(vec![format!("file on disk after Ctrl+S: {saved_text:?}")])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same check as `xtest.rs`'s own `is_balanced`: this module's coverage
    /// is scoped to its own one script, not that one's.
    fn is_balanced(script: &str) -> bool {
        let mut parens = 0i32;
        let mut braces = 0i32;
        for c in script.chars() {
            match c {
                '(' => parens += 1,
                ')' => parens -= 1,
                '{' => braces += 1,
                '}' => braces -= 1,
                _ => {}
            }
            if parens < 0 || braces < 0 {
                return false;
            }
        }
        parens == 0 && braces == 0
    }

    #[test]
    fn the_script_is_balanced() {
        assert!(is_balanced(SET_FIELD_JS), "{SET_FIELD_JS}");
    }
}
