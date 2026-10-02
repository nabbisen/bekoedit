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

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "form_field_commits_before_mode_switch";

/// Sets the paragraph field's value, never leaving it (no `change` event),
/// exactly the uncommitted state a real keystroke leaves.
const SET_FIELD_JS: &str = "const el = document.querySelector('.form-mode .paragraph-input'); \
     if (!el) return false; \
     el.focus(); \
     el.value = 'abc def'; \
     return document.activeElement === el && el.value === 'abc def';";

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

    let set_up: bool = dom::run_script(SET_FIELD_JS).await?;
    if !set_up {
        return Err(format!(
            "{NAME}: could not set the paragraph field's pending value"
        ));
    }

    // The real act under test: a real click switching to Text Mode, with
    // the Form field still focused and never blurred.
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

    let shows_typed_text = dom::editor_contains("abc def").await?;
    if !shows_typed_text {
        return Err(format!(
            "{NAME}: Text Mode does not show the typed text \"abc def\" after the mode switch"
        ));
    }

    Ok(vec![
        "Text Mode shows the typed text after the mode switch".to_string(),
    ])
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
