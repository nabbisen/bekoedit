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
//! Task 052's own run showed this scenario still fails, with a focus
//! trace of **zero** `source.focus.*` entries -- `submit_interaction`
//! was never reached at all, upstream of anything task 052 touched.
//! This is the first scenario in the project to send `ctrl+1`..`4`
//! through `xdotool key` (every earlier mode-switch scenario clicks the
//! tab); task 053 diagnoses, without changing any product code, whether
//! the keystroke is reaching `shortcuts.js`'s own listener at all, and
//! with what `key`.

use std::time::Duration;

use dioxus::desktop::DesktopContext;
use serde::Deserialize;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "keyboard_mode_switch_commits_pending_field";

/// What the field is expected to hold once the real keystrokes below
/// land: the seeded `abc`, plus the typed ` def`.
const TYPED_VALUE: &str = "abc def";

/// How long to wait after each `xdotool key` attempt before reading the
/// diagnostics back (task 053 §2.3) -- long enough for a keydown, a
/// flush and a relay round trip to have settled, short enough not to
/// bloat the scenario.
const DIAGNOSTIC_SETTLE: Duration = Duration::from_secs(1);

/// Task 053 §2.1/§2.2: installs, once, a capture-phase `keydown`
/// recorder (bounded to the last 10 events) and a counting wrapper
/// around `window.__bk_shortcut_relay` -- wrapped, never replaced, so
/// the app's own shortcut handling still runs exactly as it would
/// without this task's diagnostics. Returns `true` once both are in
/// place (or already were, on a second call).
const INSTALL_DIAGNOSTICS_JS: &str = "\
    if (!window.__bk_keydown_recorder) { \
        window.__bk_keydown_recorder = []; \
        window.addEventListener('keydown', (e) => { \
            window.__bk_keydown_recorder.push({ \
                key: e.key, code: e.code, ctrlKey: e.ctrlKey, defaultPrevented: e.defaultPrevented \
            }); \
            while (window.__bk_keydown_recorder.length > 10) { window.__bk_keydown_recorder.shift(); } \
        }, true); \
    } \
    if (!window.__bk_relay_counter) { \
        window.__bk_relay_counter = { count: 0, keys: [] }; \
        const original = window.__bk_shortcut_relay; \
        if (typeof original === 'function') { \
            window.__bk_shortcut_relay = (message) => { \
                try { \
                    window.__bk_relay_counter.count += 1; \
                    window.__bk_relay_counter.keys.push(JSON.parse(message).key); \
                } catch (error) { /* malformed message: counted below regardless */ } \
                return original(message); \
            }; \
        } \
    } \
    return true;";

/// Task 053 §2.3: the four values the task asks for, read back without
/// disturbing anything -- never the field's own commit state through a
/// side-effecting probe, only what is already there to read.
const READ_DIAGNOSTICS_JS: &str = "\
    const tabs = Array.from(document.querySelectorAll('[data-source-focus-launch^=\"mode-\"]')) \
        .map((el) => `${el.getAttribute('data-source-focus-launch')}:selected=${el.getAttribute('aria-selected')}`); \
    const field = document.querySelector('.form-mode .paragraph-input'); \
    const counter = window.__bk_relay_counter || { count: 0, keys: [] }; \
    return { \
        keydownEvents: window.__bk_keydown_recorder || [], \
        relayCount: counter.count, \
        relayKeys: counter.keys, \
        selectedTabs: tabs, \
        fieldValue: field ? field.value : null, \
    };";

// Every field here is read only through `{:?}` in `DiagnosticReport::print`
// below -- the whole point is to print exactly what the page recorded,
// not to branch on any one field in Rust.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeydownEvent {
    key: String,
    code: String,
    ctrl_key: bool,
    default_prevented: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticReport {
    keydown_events: Vec<KeydownEvent>,
    relay_count: u64,
    relay_keys: Vec<String>,
    selected_tabs: Vec<String>,
    field_value: Option<String>,
}

impl DiagnosticReport {
    /// Prints straight to the release-checks log, unconditionally --
    /// never folded into this scenario's own `Ok(checks)` vector, which
    /// a later failing assertion would discard before it was ever
    /// returned (task 053 §3: "no change to the assertion").
    fn print(&self, label: &str) {
        println!(
            "  {NAME}: after {label}: keydown events: {:?}",
            self.keydown_events
        );
        println!(
            "  {NAME}: after {label}: relay count={} keys={:?}",
            self.relay_count, self.relay_keys
        );
        println!(
            "  {NAME}: after {label}: selected tabs: {:?}",
            self.selected_tabs
        );
        println!(
            "  {NAME}: after {label}: field value: {:?}",
            self.field_value
        );
    }
}

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

    // Task 053 §2.1/§2.2: install the recorder and the relay counter
    // before the real act under test, so both see it from the start.
    let installed: bool = dom::run_script(INSTALL_DIAGNOSTICS_JS).await?;
    if !installed {
        return Err(format!(
            "{NAME}: could not install the keydown/relay diagnostics"
        ));
    }

    // The real act under test: Ctrl+1 to Text Mode, with no click at all
    // after the field was focused -- nothing in this scenario's own
    // script moves focus away from it before the queued commit runs.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+1"]).await?;
    tokio::time::sleep(DIAGNOSTIC_SETTLE).await;
    let report_1: DiagnosticReport = dom::run_script(READ_DIAGNOSTICS_JS).await?;
    report_1.print("ctrl+1");

    // Task 053 §2.4: a keypad variant, as a control -- if this one
    // reaches the listener as `key === "1"` and the main-row one above
    // did not, that alone says it is keysym delivery, not something
    // further down the relay/claim path (ctrl+s and the click-launched
    // mode switch already prove that path works).
    run_xdotool(&["key", "--clearmodifiers", "ctrl+KP_1"]).await?;
    tokio::time::sleep(DIAGNOSTIC_SETTLE).await;
    let report_2: DiagnosticReport = dom::run_script(READ_DIAGNOSTICS_JS).await?;
    report_2.print("ctrl+KP_1");

    // Then fail as today, if it fails: the original assertion, unchanged.
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

    /// Same check as `xtest.rs`'s own `is_balanced`: this module's
    /// coverage is scoped to its own two scripts, not that one's.
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
    fn both_diagnostic_scripts_are_balanced() {
        assert!(
            is_balanced(INSTALL_DIAGNOSTICS_JS),
            "{INSTALL_DIAGNOSTICS_JS}"
        );
        assert!(is_balanced(READ_DIAGNOSTICS_JS), "{READ_DIAGNOSTICS_JS}");
    }
}
