use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;
use serde::Deserialize;

use crate::persistence::AppPersistence;

const PROFILE_PREFIX: &str = "bekoedit-webview-smoke-";
const MARKER: &str = "RFC041_WEBVIEW_SMOKE_MARKER";
const EXPECTED_MILESTONES: [&str; 7] = [
    "observer_installed",
    "start_visible",
    "new_clicked",
    "editor_ready_focused",
    "edit_dispatched",
    "preview_clicked",
    "preview_verified",
];
const NOT_COMPLETE: u8 = 0;
const SUCCEEDED: u8 = 1;

static LAUNCH_CONFIG: OnceLock<LaunchConfig> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunMode {
    Normal,
    HeadlessSmoke,
    WebViewSmoke(PathBuf),
}

impl RunMode {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let args: Vec<OsString> = args.into_iter().collect();
        if args.iter().any(|arg| arg == "--headless-smoke") {
            return Ok(Self::HeadlessSmoke);
        }
        let Some(index) = args.iter().position(|arg| arg == "--webview-smoke") else {
            return Ok(Self::Normal);
        };
        if index != 0 || args.len() != 2 {
            return Err("--webview-smoke requires exactly one profile-root argument".into());
        }
        Ok(Self::WebViewSmoke(PathBuf::from(&args[1])))
    }
}

#[derive(Clone)]
pub struct LaunchConfig {
    pub persistence: AppPersistence,
    pub webview_smoke: bool,
    terminal: Option<Arc<SmokeTerminal>>,
}

pub struct SmokeRun {
    profile_root: Option<PathBuf>,
    terminal: Arc<SmokeTerminal>,
}

impl SmokeRun {
    pub fn finalize_exit_code(mut self) -> i32 {
        let succeeded = self.terminal.succeeded();
        self.cleanup();
        if succeeded {
            0
        } else {
            eprintln!("bekoedit WebView lifecycle smoke FAILED: no validated terminal success");
            1
        }
    }

    fn cleanup(&mut self) {
        let Some(profile_root) = self.profile_root.take() else {
            return;
        };
        if let Err(error) = std::fs::remove_dir_all(&profile_root) {
            eprintln!(
                "bekoedit WebView smoke warning: could not remove {}: {error}",
                profile_root.display()
            );
        }
    }
}

impl Drop for SmokeRun {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub fn prepare_launch(run_mode: RunMode) -> Result<Option<SmokeRun>, String> {
    let (config, smoke_run) = match run_mode {
        RunMode::HeadlessSmoke => {
            return Err("headless smoke must run before desktop launch preparation".into());
        }
        RunMode::Normal => (
            LaunchConfig {
                persistence: AppPersistence::platform_default(),
                webview_smoke: false,
                terminal: None,
            },
            None,
        ),
        RunMode::WebViewSmoke(requested_root) => {
            let profile = SmokeProfile::create(&requested_root)?;
            let terminal = Arc::new(SmokeTerminal::default());
            let config = LaunchConfig {
                persistence: profile.persistence.clone(),
                webview_smoke: true,
                terminal: Some(terminal.clone()),
            };
            let run = SmokeRun {
                profile_root: Some(profile.root),
                terminal,
            };
            (config, Some(run))
        }
    };
    LAUNCH_CONFIG
        .set(config)
        .map_err(|_| "desktop launch configuration was already installed".to_string())?;
    Ok(smoke_run)
}

pub fn launch_config() -> &'static LaunchConfig {
    LAUNCH_CONFIG
        .get()
        .expect("main installs launch configuration before Dioxus starts")
}

struct SmokeProfile {
    root: PathBuf,
    persistence: AppPersistence,
}

impl SmokeProfile {
    fn create(requested_root: &Path) -> Result<Self, String> {
        if !requested_root.is_absolute() {
            return Err("WebView smoke profile root must be absolute".into());
        }
        let safe_name = requested_root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(PROFILE_PREFIX));
        if !safe_name {
            return Err(format!(
                "WebView smoke profile name must start with {PROFILE_PREFIX}"
            ));
        }
        if requested_root.exists() {
            return Err("WebView smoke profile root must not already exist".into());
        }
        let parent = requested_root
            .parent()
            .ok_or_else(|| "WebView smoke profile root has no parent".to_string())?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|error| format!("cannot resolve smoke profile parent: {error}"))?;
        std::fs::create_dir(requested_root)
            .map_err(|error| format!("cannot create smoke profile root: {error}"))?;
        let root = requested_root.canonicalize().map_err(|error| {
            let _ = std::fs::remove_dir_all(requested_root);
            format!("cannot resolve smoke profile root: {error}")
        })?;
        if root.parent() != Some(canonical_parent.as_path()) {
            let _ = std::fs::remove_dir_all(&root);
            return Err("WebView smoke profile escaped its requested parent".into());
        }
        let persistence = AppPersistence::isolated(root.clone());
        let paths = persistence
            .isolated_paths()
            .expect("isolated persistence has paths");
        if !paths.all_within_root() {
            let _ = std::fs::remove_dir_all(&root);
            return Err("WebView smoke persistence escaped its profile root".into());
        }
        for directory in [
            paths.settings_file().parent(),
            paths.recents_file().parent(),
            Some(paths.recovery_dir()),
            Some(paths.history_dir()),
        ] {
            let Some(directory) = directory else {
                let _ = std::fs::remove_dir_all(&root);
                return Err("WebView smoke persistence path has no parent".into());
            };
            if let Err(error) = std::fs::create_dir_all(directory) {
                let _ = std::fs::remove_dir_all(&root);
                return Err(format!("cannot initialize smoke profile: {error}"));
            }
        }
        Ok(Self { root, persistence })
    }
}

#[derive(Debug, Default)]
struct SmokeTerminal {
    state: AtomicU8,
}

impl SmokeTerminal {
    fn accept(&self, result: &DriverResult) -> Result<(), String> {
        validate_driver_result(result)?;
        self.state
            .compare_exchange(NOT_COMPLETE, SUCCEEDED, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "WebView smoke terminal result was already recorded".to_string())?;
        Ok(())
    }

    fn succeeded(&self) -> bool {
        self.state.load(Ordering::SeqCst) == SUCCEEDED
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriverResult {
    ok: bool,
    stage: String,
    marker: String,
    milestones: Vec<String>,
    error_toast_seen: bool,
    error: Option<String>,
}

fn validate_driver_result(result: &DriverResult) -> Result<(), String> {
    if !result.ok {
        return Err(format!(
            "driver failed at {}: {}",
            result.stage,
            result.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if result.stage != "preview_verified" || result.marker != MARKER {
        return Err("driver returned the wrong terminal stage or marker".into());
    }
    if result.error_toast_seen {
        return Err("an error toast appeared during the WebView smoke sequence".into());
    }
    if result.error.is_some() {
        return Err("successful driver result unexpectedly contained an error".into());
    }
    if result
        .milestones
        .iter()
        .map(String::as_str)
        .ne(EXPECTED_MILESTONES)
    {
        return Err("driver returned an incomplete or out-of-order milestone list".into());
    }
    Ok(())
}

#[component]
pub fn WebViewSmokeDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let terminal = launch_config()
        .terminal
        .clone()
        .expect("smoke driver requires a terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            println!("bekoedit WebView lifecycle smoke");
            let mut eval = document::eval(WEBVIEW_SMOKE_JS);
            match eval.recv::<DriverResult>().await {
                Ok(result) => match terminal.accept(&result) {
                    Ok(()) => {
                        for milestone in &result.milestones {
                            println!("  ✓ {milestone}");
                        }
                        println!("bekoedit WebView lifecycle smoke PASSED");
                    }
                    Err(error) => eprintln!("bekoedit WebView lifecycle smoke FAILED: {error}"),
                },
                Err(error) => {
                    eprintln!("bekoedit WebView lifecycle smoke FAILED: evaluator error: {error}")
                }
            }
            desktop.close();
        }
    });
    rsx! {}
}

const WEBVIEW_SMOKE_JS: &str = include_str!("webview_smoke/driver.js");

#[cfg(test)]
mod tests;
