//! Task 047 Part A / task 048 §2.4, scenario `toolbar_probe`: a Form Mode
//! toolbar click must commit the field's pending text along with the
//! toggle, not just the document's last-committed text.
//!
//! Originally a *reporting* scenario (task 047 Part A): it set the field's
//! pending value and selection, clicked **B**, and reported what was saved
//! without asserting anything. Its first real run, `36975693053` on
//! `1492cac`, confirmed the hypothesis the hard way -- the toolbar click
//! committed neither the pending text nor the toggle; Ctrl+S wrote back
//! exactly the seeded `abc\n`, with the field still *showing* `abc def` on
//! screen. Task 048 D2 fixes this: `resolve_toggle_inline` resolves against
//! the field's own current value, sent with the selection, as one patch
//! (D1, the keyboard-command path, is a separate mechanism --
//! `shortcuts.js` flushing a focused field before relaying, task 051 --
//! that `save_pending_field`/`keyboard_mode_switch_commits_pending_field`
//! cover instead), so this scenario now asserts the fixed bytes instead of
//! merely reporting them.
//!
//! Sets the field's value and selection from script -- which, like typing,
//! fires no `change` event, exactly the uncommitted state a real keystroke
//! leaves -- then sends a real click at the **B** button, a real Ctrl+S,
//! and asserts the saved bytes.

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

/// The fixed, correct outcome: the pending " def" is committed, and the
/// toggle wraps exactly what was selected (task 048 D1+D2, one patch).
const EXPECTED_SAVED_TEXT: &str = "abc **def**\n";

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
    if saved_text != EXPECTED_SAVED_TEXT {
        return Err(format!(
            "{NAME}: expected the saved bytes to be {EXPECTED_SAVED_TEXT:?} (the pending \
             edit committed, then the toggle applied, as one patch), got {saved_text:?}; the \
             field showed {field_value_after_click:?} right after the click"
        ));
    }

    Ok(vec![
        format!("field value right after the click: {field_value_after_click:?}"),
        format!("file on disk after Ctrl+S: {saved_text:?}"),
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
