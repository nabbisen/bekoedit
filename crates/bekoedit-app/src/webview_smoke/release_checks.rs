//! Task 026: a fourth WebView run -- the release walkthrough's phase-1 checks
//! #2, #3, #4 and #8, by machine, on the release binary.
//!
//! `--webview-release-checks <profile-root> <scenario>`: **one process launch
//! per scenario**, because these are launch-time behaviours and one scenario's
//! state must not reach the next. Each scenario seeds its own isolated profile
//! (`seed.rs`), so no native dialog is ever involved.
//!
//! **How "the Start Screen never mounted" is observed from the first render.**
//! A driver injected after first render cannot see a mount that has already
//! come and gone. So the observation is a fact the app records:
//! `StartScreen` calls `webview_smoke::note_start_screen_mounted` in a
//! `use_hook`, which counts mounts in a static, and only in a run that has a
//! `release_checks` launch config. `create_app_state` decides and opens the
//! workspace before the first render (`app.rs`, inside `use_hook`), so a
//! reopen that works never mounts the Start Screen at all.
//!
//! Everything else is read from the same signals the app renders from
//! (`AppState`, the toast list), plus a DOM snapshot for what is on screen.
//! Assertions are pure functions (`launch.rs`, `bytes.rs`) that name the
//! scenario, the check and what they saw; none of them is a timeout.

use std::sync::atomic::{AtomicU8, Ordering};

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::components::toast::Toast;
use crate::i18n::Lang;

mod bytes;
mod dom;
mod launch;
mod link_clicks;
mod link_judge;
mod link_layer_two;
mod mode_switch;
mod paste_probe;
mod paste_report;
mod save;
mod seed;
pub(super) use seed::prepare;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseScenario {
    ReopenUsable,
    ReopenMissing,
    ReopenDisabled,
    SavePreservesBytes,
    /// Task 027: the same edit-and-save over a uniform-CRLF file (Rule 1).
    SavePreservesCrlfBytes,
    /// Task 027: open a CRLF file in Text Mode, make no edit, switch to
    /// Preview, wait past autosave: the bytes must not change.
    ModeSwitchPreservesBytes,
    /// Task 033: only `http(s):` and `mailto:` links ever reach an OS opener,
    /// on the release binary, with a stub opener.
    LinkClicksReachOnlyTheBrowser,
    /// RFC-046 slice 2, part A: what the WebView does with a real paste. Reports;
    /// asserts no product behaviour.
    PasteProbe,
}

impl ReleaseScenario {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReopenUsable => "reopen_usable",
            Self::ReopenMissing => "reopen_missing",
            Self::ReopenDisabled => "reopen_disabled",
            Self::SavePreservesBytes => "save_preserves_bytes",
            Self::SavePreservesCrlfBytes => "save_preserves_crlf_bytes",
            Self::ModeSwitchPreservesBytes => "mode_switch_preserves_bytes",
            Self::LinkClicksReachOnlyTheBrowser => link_judge::NAME,
            Self::PasteProbe => paste_probe::NAME,
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        [
            Self::ReopenUsable,
            Self::ReopenMissing,
            Self::ReopenDisabled,
            Self::SavePreservesBytes,
            Self::SavePreservesCrlfBytes,
            Self::ModeSwitchPreservesBytes,
            Self::LinkClicksReachOnlyTheBrowser,
            Self::PasteProbe,
        ]
        .into_iter()
        .find(|scenario| scenario.name() == name)
        .ok_or_else(|| {
            format!(
                "unknown release-checks scenario {name:?}; expected reopen_usable, \
                 reopen_missing, reopen_disabled, save_preserves_bytes, \
                 save_preserves_crlf_bytes, mode_switch_preserves_bytes link_clicks_reach_only_the_browser or \
                 paste_probe"
            )
        })
    }
}

/// What a scenario's seed put on disk, so the assertions know what to expect.
#[derive(Debug, Clone)]
pub struct Expectation {
    pub workspace: std::path::PathBuf,
    pub display_name: String,
    /// `reopen_missing`: the older, still-usable recent entry that must NOT
    /// be opened in place of the missing one.
    pub older_workspace: Option<std::path::PathBuf>,
    /// `save_preserves_bytes`: the seeded file and its exact original bytes.
    pub file: Option<std::path::PathBuf>,
    pub original: Vec<u8>,
    /// `link_clicks_reach_only_the_browser`: the opener stub's log, whose path
    /// CI passes in `BEKOEDIT_LINK_OPENER_LOG`.
    pub opener_log: Option<std::path::PathBuf>,
}

#[derive(Debug)]
pub struct ReleaseChecksTerminal {
    state: AtomicU8,
    pub scenario: ReleaseScenario,
    pub expectation: Expectation,
}

impl ReleaseChecksTerminal {
    pub(super) fn new(scenario: ReleaseScenario, expectation: Expectation) -> Self {
        Self {
            state: AtomicU8::new(0),
            scenario,
            expectation,
        }
    }

    fn accept(&self) -> Result<(), String> {
        self.state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "release-checks result was already recorded".to_string())?;
        Ok(())
    }

    pub(super) fn succeeded(&self) -> bool {
        self.state.load(Ordering::SeqCst) == 1
    }
}

#[component]
pub fn WebViewReleaseChecksDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let state = use_context::<Signal<AppState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let lang = use_context::<Signal<Lang>>();
    let terminal = super::launch_config()
        .release_checks
        .clone()
        .expect("release-checks driver requires terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            let name = terminal.scenario.name();
            println!("bekoedit task 026 release-checks run: {name}");
            let lang = *lang.peek();
            let outcome = match terminal.scenario {
                ReleaseScenario::SavePreservesBytes | ReleaseScenario::SavePreservesCrlfBytes => {
                    save::run(&terminal, &desktop, state).await
                }
                ReleaseScenario::ModeSwitchPreservesBytes => {
                    mode_switch::run(&terminal, &desktop, state).await
                }
                ReleaseScenario::LinkClicksReachOnlyTheBrowser => {
                    link_clicks::run(&terminal, &desktop, toasts, lang).await
                }
                ReleaseScenario::PasteProbe => paste_probe::run(&terminal, &desktop).await,
                _ => launch::run(&terminal, state, toasts, lang).await,
            };
            match outcome.and_then(|checks| terminal.accept().map(|()| checks)) {
                Ok(checks) => {
                    for check in &checks {
                        println!("  ✓ {name}: {check}");
                    }
                    println!("bekoedit task 026 release-checks run PASSED: {name}");
                }
                Err(error) => {
                    eprintln!("bekoedit task 026 release-checks run FAILED: {error}");
                }
            }
            desktop.close();
        }
    });
    rsx! {}
}

#[cfg(test)]
mod layer_two_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod paste_report_tests;
#[cfg(test)]
mod tests;
