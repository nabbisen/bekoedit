//! Task 048 §2.4's third assertion: a pending Form Mode field, then a
//! **mode switch** to Text Mode (no save, no toolbar click) -- Text Mode
//! must show the typed text.
//!
//! Switching *into* Text Mode claims editor focus
//! (`source_sync::focus::focus_target`), so this exercises the async
//! focus-guard-arming path (`submit_interaction`'s own `spawn`), not the
//! direct `submit_source_command` path `save_pending_field`/`toolbar_probe`
//! exercise -- a different route to the same commit-before-command
//! ordering (task 048 D1), proven by the same real WebView run as the
//! other two.
//!
//! Task 049 §2.1: the mode tab's own click moves DOM focus to the tab on
//! `mousedown`, before any script runs -- by the time the queued
//! `commit_pending_form_field` read executes, `document.activeElement` is
//! already the tab, not the field, so that path has nothing to commit, by
//! design. What this scenario actually needs is the field's *own*
//! blur-then-`change` commit (`block_view.rs`'s `onchange`), which the
//! review's reading of the failed run named as this scenario's real bug:
//! per the HTML spec, `change` fires on blur only if the element's value
//! was changed by the user since it was focused -- a value set from
//! script (the previous version of this file) never sets that flag, so
//! the blur commits nothing, and the scenario was not testing what it
//! claimed to. Real keystrokes (`xdotool type`) set it, exactly as a real
//! user's typing would.

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "form_field_commits_before_mode_switch";

/// Exactly what the field is expected to hold once the real keystrokes
/// below land: the seeded `abc`, plus the typed ` def`.
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
    // field's `.value` set from script (task 049 §2.1's fix: see the
    // module doc comment for why that never exercised a real commit).
    click_via_xtest(desktop, ".form-mode .paragraph-input", None, 0).await?;
    run_xdotool(&["key", "--clearmodifiers", "End"]).await?;
    run_xdotool(&["type", "--clearmodifiers", " def"]).await?;
    wait_until(
        NAME,
        "the paragraph field to hold the typed text",
        || async { dom::paragraph_field_value_is(TYPED_VALUE).await },
    )
    .await?;

    // The real act under test: a real click switching to Text Mode, with
    // the Form field still focused and never blurred by anything but
    // this click.
    click_via_xtest(
        desktop,
        r#"[data-source-focus-launch="mode-text"]"#,
        None,
        0,
    )
    .await?;
    wait_until(
        NAME,
        "the editor to open the file and take focus",
        || async { dom::editor_focused().await },
    )
    .await?;

    let shows_typed_text = dom::editor_contains(TYPED_VALUE).await?;
    if !shows_typed_text {
        return Err(format!(
            "{NAME}: Text Mode does not show the typed text {TYPED_VALUE:?} after the mode switch"
        ));
    }

    Ok(vec![
        "Text Mode shows the typed text after the mode switch".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seeded paragraph is `abc\n` (`seed.rs`'s `original_paragraph_note`);
    /// `End` then ` def` must land exactly on `TYPED_VALUE`, independent of a
    /// live WebView, since a wrong constant here would make the whole
    /// scenario probe the wrong text without ever failing.
    #[test]
    fn the_seeded_text_plus_the_typed_suffix_is_the_expected_value() {
        assert_eq!(format!("{}{}", "abc", " def"), TYPED_VALUE);
    }
}
