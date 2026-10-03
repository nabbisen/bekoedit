//! Task 050 §2.2: `save_pending_field`'s own keyboard path (Ctrl+S), but
//! for a mode switch -- the gap the task 049 review named.
//! `form_field_commits_before_mode_switch` already covers a mode switch
//! launched by a **click** on the mode tab, which moves DOM focus to the
//! tab on `mousedown` before any read can run and passes through the
//! field's own blur-then-`change`, never through
//! `commit_pending_form_field`'s queued commit at all. This scenario
//! switches mode with **Ctrl+1** instead: no click anywhere after the
//! field is focused, so nothing in the scenario's own script moves
//! focus away from it -- the field should still have it when the queued
//! commit's read runs, which is exactly what `save_pending_field`'s own
//! trace (task 049) showed was *not* true for that scenario's Ctrl+S.
//!
//! Task 053 diagnosed why this failed through task 052: Ctrl+1 reached
//! the page correctly (`key: "1"`, `ctrlKey: true`), but
//! `shortcuts.js`'s own relay never ran, gated on `window.dioxus` --
//! which nothing has ever defined. Task 054 removes that gate (and
//! task 053's diagnostics, their job done); this scenario needs no
//! further change.

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "keyboard_mode_switch_commits_pending_field";

/// What the field is expected to hold once the real keystrokes below
/// land: the seeded `abc`, plus the typed ` def`.
const TYPED_VALUE: &str = "abc def";

pub(super) async fn run(
    _terminal: &ReleaseChecksTerminal,
    desktop: &DesktopContext,
) -> Result<Vec<String>, String> {
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
    // field's `.value` set from script (task 049/050's own finding:
    // that is never a valid model for a pending-text scenario).
    click_via_xtest(desktop, ".form-mode .paragraph-input", None, 0).await?;
    run_xdotool(&["key", "--clearmodifiers", "End"]).await?;
    run_xdotool(&["type", "--clearmodifiers", " def"]).await?;
    wait_until(
        NAME,
        "the paragraph field to hold the typed text",
        || async { dom::paragraph_field_value_is(TYPED_VALUE).await },
    )
    .await?;

    // The real act under test: Ctrl+1 to Text Mode, with no click at all
    // after the field was focused -- nothing in this scenario's own
    // script moves focus away from it before the queued commit runs.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+1"]).await?;
    wait_until(
        NAME,
        "the editor to open the file and take focus",
        || async { dom::editor_focused().await },
    )
    .await?;

    let shows_typed_text = dom::editor_contains(TYPED_VALUE).await?;
    if !shows_typed_text {
        return Err(format!(
            "{NAME}: Text Mode does not show the typed text {TYPED_VALUE:?} after the \
             keyboard mode switch"
        ));
    }

    Ok(vec![
        "Text Mode shows the typed text after the keyboard mode switch".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seeded paragraph is `abc\n` (`seed.rs`'s `original_paragraph_note`);
    /// `End` then ` def` must land exactly on `TYPED_VALUE`, independent of a
    /// live WebView, since a wrong constant here would make the whole
    /// scenario probe the wrong text without ever failing. Also pins
    /// that `TYPED_VALUE` is not the untyped seed itself (task 050 §4's
    /// required mutation): if the commit never ran, Text Mode would
    /// still show only `abc`, and this assertion must tell the two apart.
    #[test]
    fn the_seeded_text_plus_the_typed_suffix_is_the_expected_value() {
        assert_eq!(format!("{}{}", "abc", " def"), TYPED_VALUE);
        assert_ne!("abc", TYPED_VALUE);
    }
}
