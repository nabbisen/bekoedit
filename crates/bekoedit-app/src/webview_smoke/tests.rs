use std::ffi::OsString;

use super::*;

fn successful_result() -> DriverResult {
    DriverResult {
        ok: true,
        stage: "preview_verified".into(),
        marker: MARKER.into(),
        milestones: EXPECTED_MILESTONES
            .iter()
            .map(|item| (*item).into())
            .collect(),
        error_toast_seen: false,
        error: None,
    }
}

#[test]
fn run_mode_requires_exact_webview_profile_argument() {
    assert_eq!(
        RunMode::parse(Vec::<OsString>::new()).unwrap(),
        RunMode::Normal
    );
    assert_eq!(
        RunMode::parse([OsString::from("--headless-smoke")]).unwrap(),
        RunMode::HeadlessSmoke
    );
    assert!(RunMode::parse([OsString::from("--webview-smoke")]).is_err());
    let path = PathBuf::from("/tmp/bekoedit-webview-smoke-test");
    assert_eq!(
        RunMode::parse([
            OsString::from("--webview-smoke"),
            path.clone().into_os_string()
        ])
        .unwrap(),
        RunMode::WebViewSmoke(path)
    );
}

#[test]
fn profile_rejects_relative_wrong_named_and_existing_roots() {
    assert!(SmokeProfile::create(Path::new("relative")).is_err());
    let parent = tempfile::tempdir().unwrap();
    assert!(SmokeProfile::create(&parent.path().join("unsafe-name")).is_err());
    let existing = parent.path().join("bekoedit-webview-smoke-existing");
    std::fs::create_dir(&existing).unwrap();
    assert!(SmokeProfile::create(&existing).is_err());
}

#[test]
fn profile_creates_owned_isolated_paths_and_cleanup_is_bounded() {
    let parent = tempfile::tempdir().unwrap();
    let requested = parent.path().join("bekoedit-webview-smoke-owned");
    let profile = SmokeProfile::create(&requested).unwrap();
    let paths = profile.persistence.isolated_paths().unwrap();
    assert!(paths.all_within_root());
    assert!(paths.settings_file().parent().unwrap().exists());
    assert!(paths.recovery_dir().exists());
    assert!(paths.history_dir().exists());
    let root = profile.root;
    std::fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
    assert!(parent.path().exists());
}

#[test]
fn terminal_is_fail_closed_and_only_exact_success_transitions_once() {
    let terminal = SmokeTerminal::default();
    assert!(!terminal.succeeded(), "no result and unexpected close fail");

    let mut failed = successful_result();
    failed.ok = false;
    failed.error = Some("explicit failure".into());
    assert!(terminal.accept(&failed).is_err());
    assert!(!terminal.succeeded());

    let success = successful_result();
    assert!(terminal.accept(&success).is_ok());
    assert!(terminal.succeeded());
    assert!(
        terminal.accept(&success).is_err(),
        "duplicates are rejected"
    );
    assert!(terminal.succeeded(), "late results cannot reverse success");
}

#[test]
fn smoke_run_maps_no_result_and_exact_success_to_process_codes() {
    let parent = tempfile::tempdir().unwrap();
    let failed_root = parent.path().join("bekoedit-webview-smoke-no-result");
    std::fs::create_dir(&failed_root).unwrap();
    let failed = SmokeRun {
        profile_root: Some(failed_root.clone()),
        terminal: Arc::new(SmokeTerminal::default()),
    };
    assert_eq!(failed.finalize_exit_code(), 1);
    assert!(!failed_root.exists());

    let passed_root = parent.path().join("bekoedit-webview-smoke-success");
    std::fs::create_dir(&passed_root).unwrap();
    let terminal = Arc::new(SmokeTerminal::default());
    terminal.accept(&successful_result()).unwrap();
    let passed = SmokeRun {
        profile_root: Some(passed_root.clone()),
        terminal,
    };
    assert_eq!(passed.finalize_exit_code(), 0);
    assert!(!passed_root.exists());
}

#[test]
fn smoke_run_drop_cleans_profile_before_event_loop_initialization() {
    let parent = tempfile::tempdir().unwrap();
    let profile_root = parent.path().join("bekoedit-webview-smoke-init-panic");
    std::fs::create_dir(&profile_root).unwrap();
    let run = SmokeRun {
        profile_root: Some(profile_root.clone()),
        terminal: Arc::new(SmokeTerminal::default()),
    };
    drop(run);
    assert!(!profile_root.exists());
    assert!(parent.path().exists());
}

#[test]
fn terminal_rejects_wrong_order_marker_stage_and_transient_error() {
    let mutations = [
        |result: &mut DriverResult| result.milestones.swap(0, 1),
        |result: &mut DriverResult| result.marker = "wrong".into(),
        |result: &mut DriverResult| result.stage = "preview_clicked".into(),
        |result: &mut DriverResult| result.error_toast_seen = true,
        |result: &mut DriverResult| result.error = Some("contradictory success".into()),
    ];
    for mutate in mutations {
        let terminal = SmokeTerminal::default();
        let mut result = successful_result();
        mutate(&mut result);
        assert!(terminal.accept(&result).is_err());
        assert!(!terminal.succeeded());
    }
}

#[test]
fn driver_contract_is_bounded_observable_and_uses_rendered_controls() {
    assert!(WEBVIEW_SMOKE_JS.contains("performance.now() + 15000"));
    assert!(WEBVIEW_SMOKE_JS.contains("MutationObserver"));
    assert!(WEBVIEW_SMOKE_JS.contains("errorToastSeen"));
    assert!(WEBVIEW_SMOKE_JS.contains("start-new"));
    assert!(WEBVIEW_SMOKE_JS.contains("current.hasFocus"));
    assert!(WEBVIEW_SMOKE_JS.contains("view.dispatch"));
    assert!(WEBVIEW_SMOKE_JS.contains("mode-preview"));
    assert!(WEBVIEW_SMOKE_JS.contains("article.preview"));
    assert!(WEBVIEW_SMOKE_JS.contains("preview_verified"));
}
