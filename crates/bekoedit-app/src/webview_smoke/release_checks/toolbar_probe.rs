//! Task 047 Part A, scenario `toolbar_probe`: does a Form Mode toolbar
//! click commit the field's pending text along with the toggle, or only
//! the document's last-committed text?
//!
//! The hypothesis, from reading only -- it is not a finding:
//! 1. A Form Mode field commits its text on `onchange`, which fires only
//!    when the field loses focus.
//! 2. The toolbar's buttons `preventDefault` on `mousedown` precisely so
//!    the field keeps focus -- `onchange` therefore never fires before the
//!    click's own toggle is resolved.
//! 3. The toggle is resolved against the *document's* text (the last
//!    `ReplacePlainText` commit), not whatever the user just typed. The
//!    browser's selection offsets, though, refer to the typed text.
//!
//! If that is right, a user who types into a paragraph, selects part of
//! what they just typed, and clicks a toolbar button loses that typed text
//! (or gets markers at the wrong byte positions) once the document
//! re-renders the field from the committed source.
//!
//! This scenario sets the field's value and selection from script -- which,
//! like typing, fires no `change` event, exactly the uncommitted state the
//! hypothesis describes -- then sends a real click at the **B** button, and
//! reports what the field shows and what Ctrl+S actually saves. It asserts
//! no product behaviour: it reports, so the hypothesis can be confirmed or
//! not confirmed from real CI evidence before task 047 Part A2 is written
//! (task 047 §1, "I write the fix only once the cause is shown").

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "toolbar_probe";

/// Sets the paragraph field to this value and selects `def` (UTF-16 offsets
/// 4..7) -- never leaving the field, so no `change` event fires, exactly
/// what step 2 of the hypothesis needs the field to be in.
const SET_FIELD_JS: &str = "const el = document.querySelector('.form-mode .paragraph-input'); \
     if (!el) return false; \
     el.focus(); \
     el.value = 'abc def'; \
     el.setSelectionRange(4, 7); \
     return document.activeElement === el && el.value === 'abc def';";

const READ_FIELD_JS: &str = "const el = document.querySelector('.form-mode .paragraph-input'); return el ? el.value : null;";

/// What a save settles to within, past the autosave/Ctrl+S write -- the same
/// margin `save.rs`'s own poll uses.
const SAVE_DEADLINE: Duration = Duration::from_secs(10);

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
            "{NAME}: could not set the paragraph field's pending value and selection"
        ));
    }

    // The real click under test: the field above keeps focus throughout
    // (mousedown's own `preventDefault`), so nothing but this click's own
    // handler can commit or discard the pending value.
    click_via_xtest(desktop, ".inline-fmt-btn", Some("B"), 0).await?;

    let field_value_after_click: Option<String> = dom::run_script(READ_FIELD_JS).await?;

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

    // What the hypothesis predicts if it is right: the toggle landed on
    // "abc" (the document's last commit), not on the pending "def" the
    // selection pointed at, and the typed " def" is nowhere on disk.
    let typed_text_survived = saved_text.contains("def");
    let toggle_hit_the_selection = saved_text.contains("**def**");

    Ok(vec![
        format!("field value right after the click: {field_value_after_click:?}"),
        format!("file on disk after Ctrl+S: {saved_text:?}"),
        format!(
            "hypothesis (the click loses the pending edit): {}",
            if !toggle_hit_the_selection {
                "confirmed"
            } else {
                "not confirmed"
            }
        ),
        format!("the typed text survived onto disk at all: {typed_text_survived}"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same check as `xtest.rs`'s own `is_balanced`: this module's coverage
    /// is scoped to its own two scripts, not that one's.
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
    fn both_scripts_are_balanced() {
        assert!(is_balanced(SET_FIELD_JS), "{SET_FIELD_JS}");
        assert!(is_balanced(READ_FIELD_JS), "{READ_FIELD_JS}");
    }

    /// The selection `SET_FIELD_JS` asks for (UTF-16 offsets 4..7) must be
    /// exactly "def" within "abc def" -- checked in Rust, independent of a
    /// live WebView, since a wrong offset here would make the whole
    /// scenario probe the wrong thing without ever failing.
    #[test]
    fn the_selection_offsets_name_exactly_def() {
        const FIELD_VALUE: &str = "abc def";
        assert!(SET_FIELD_JS.contains("el.value = 'abc def'"));
        assert!(SET_FIELD_JS.contains("el.setSelectionRange(4, 7)"));
        assert_eq!(&FIELD_VALUE[4..7], "def");
    }
}
