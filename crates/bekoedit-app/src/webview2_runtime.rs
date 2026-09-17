//! Task 020: on Windows, check that the WebView2 runtime is available before
//! Dioxus launches, and say so plainly when it is not.
//!
//! Without the runtime, wry fails to create the WebView2 environment and
//! dioxus-desktop unwraps that error after the window already exists: the
//! window flashes and the process panics, with nothing a user can read
//! (the release binary has no console, task 018). Instead, a native message
//! box names what is missing and where to get it, and the process exits with
//! `EXIT_WEBVIEW2_RUNTIME_MISSING`.
//!
//! The decision is a pure function so the missing branch is testable without
//! Windows; the probe and the dialog are thin effects around it. There is no
//! switch in the shipped binary that fakes a missing runtime.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::i18n::{Lang, tr};

/// The exit code when the WebView2 runtime is missing. Distinct from 1 (a
/// failed smoke run) and 2 (an argument error).
pub const EXIT_WEBVIEW2_RUNTIME_MISSING: i32 = 3;

/// Microsoft's WebView2 download page.
pub const WEBVIEW2_DOWNLOAD_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDecision {
    Launch,
    ReportMissing(MissingRuntimeReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingRuntimeReport {
    pub title: &'static str,
    pub message: String,
    pub exit_code: i32,
}

/// Decides from a runtime probe's result. `lang` is only asked for when the
/// runtime is missing, so a normal launch reads no settings and adds no delay.
pub fn decide<E>(probe: Result<String, E>, lang: impl FnOnce() -> Lang) -> RuntimeDecision {
    match probe {
        Ok(_) => RuntimeDecision::Launch,
        Err(_) => {
            let lang = lang();
            RuntimeDecision::ReportMissing(MissingRuntimeReport {
                title: tr(lang, "webview2.missing.title"),
                message: format!(
                    "{}\n\n{}",
                    tr(lang, "webview2.missing.body"),
                    WEBVIEW2_DOWNLOAD_URL
                ),
                exit_code: EXIT_WEBVIEW2_RUNTIME_MISSING,
            })
        }
    }
}

/// Probes for the runtime and returns if it is available. Otherwise shows a
/// native message box and exits. The language comes from the saved settings;
/// `AppSettings::load` falls back to defaults (English) on any read or parse
/// error, so settings can never fail the launch.
#[cfg(windows)]
pub fn ensure_runtime_or_exit() {
    let decision = decide(dioxus::desktop::wry::webview_version(), || {
        crate::persistence::AppPersistence::platform_default()
            .load_settings()
            .lang
    });
    if let RuntimeDecision::ReportMissing(report) = decision {
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title(report.title)
            .set_description(report.message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        std::process::exit(report.exit_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_present_runtime_launches_without_reading_the_language() {
        let decision = decide(Ok::<_, ()>("130.0.2849.80".into()), || {
            panic!("a normal launch must not read settings")
        });
        assert_eq!(decision, RuntimeDecision::Launch);
    }

    fn report_for(lang: Lang) -> MissingRuntimeReport {
        match decide(Err::<String, _>("no WebView2 runtime"), || lang) {
            RuntimeDecision::ReportMissing(report) => report,
            RuntimeDecision::Launch => panic!("a probe error must not launch"),
        }
    }

    #[test]
    fn a_missing_runtime_is_reported_in_english_with_the_download_page() {
        let report = report_for(Lang::En);
        assert_eq!(report.title, tr(Lang::En, "webview2.missing.title"));
        assert!(report.message.contains("Microsoft Edge WebView2 Runtime"));
        assert!(report.message.contains("not installed"));
        assert!(report.message.ends_with(WEBVIEW2_DOWNLOAD_URL));
        assert_eq!(report.exit_code, EXIT_WEBVIEW2_RUNTIME_MISSING);
    }

    #[test]
    fn a_missing_runtime_is_reported_in_japanese_with_the_download_page() {
        let report = report_for(Lang::Ja);
        assert_eq!(report.title, tr(Lang::Ja, "webview2.missing.title"));
        assert!(report.message.contains("Microsoft Edge WebView2 Runtime"));
        assert!(report.message.contains("インストールされていません"));
        assert!(report.message.ends_with(WEBVIEW2_DOWNLOAD_URL));
        assert_ne!(report.title, tr(Lang::En, "webview2.missing.title"));
    }

    #[test]
    fn the_exit_code_is_distinct_and_is_the_one_main_exits_with() {
        assert!(![0, 1, 2].contains(&EXIT_WEBVIEW2_RUNTIME_MISSING));
        assert_eq!(
            WEBVIEW2_DOWNLOAD_URL,
            "https://developer.microsoft.com/microsoft-edge/webview2/"
        );
        // One constant: the report carries it, and the effect exits with the
        // report's code rather than a literal of its own.
        let source = include_str!("webview2_runtime.rs");
        let effect = source
            .split("pub fn ensure_runtime_or_exit()")
            .nth(1)
            .and_then(|rest| rest.split("#[cfg(test)]").next())
            .expect("ensure_runtime_or_exit");
        assert!(effect.contains("std::process::exit(report.exit_code)"));
        assert!(!effect.contains("exit(3)"));
        // main runs the check for the normal launch only, before Dioxus.
        let main = include_str!("main.rs");
        let check = main
            .find("webview2_runtime::ensure_runtime_or_exit()")
            .expect("main calls the check");
        let normal = main[..check]
            .rfind("run_mode == webview_smoke::RunMode::Normal")
            .expect("normal launch only");
        let launch = main
            .find("dioxus::LaunchBuilder::desktop()")
            .expect("launch");
        assert!(normal < check && check < launch);
    }

    /// Task 020 §4.3: the real probe on a real Windows machine, so a working
    /// install is never wrongly blocked. CI's Windows job runs it by name.
    #[cfg(windows)]
    #[test]
    fn real_webview2_probe_finds_the_runtime_on_this_machine() {
        let version = dioxus::desktop::wry::webview_version()
            .expect("the WebView2 runtime is installed on this machine");
        println!("WebView2 runtime version on this machine: {version}");
        assert!(!version.is_empty());
        assert_eq!(
            decide(Ok::<_, ()>(version), || Lang::En),
            RuntimeDecision::Launch
        );
    }
}
