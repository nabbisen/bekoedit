//! Task 033: the stub openers really are the only openers `webbrowser` can
//! reach on Linux, and they see what the scenario expects them to see.
//!
//! `link_clicks_reach_only_the_browser` (a WebView run, CI only) leans on
//! three facts about `webbrowser` 1.2.4, and this test runs them against the
//! real crate and the real `scripts/webview-link-opener-stubs.sh`, with no
//! WebView and no display:
//!
//! 1. with `BROWSER` set to the stub, `open(url)` hands the stub exactly `url`,
//!    for `https:` and `mailto:` alike, and nothing else runs;
//! 2. a **relative path**, the leak task 032 closed, reaches the same stub as a
//!    `file://<cwd>/…` URL, so a regression would show in the log;
//! 3. with `BROWSER` unset, every fallback opener is a shim that logs
//!    `UNEXPECTED-OPENER`, so nothing can open a real browser around the stub.
//!
//! One test in this file, because it sets process environment variables.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn lines(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// The stubs run in the background, so wait for `count` lines.
fn wait_for_lines(log: &Path, count: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let now = lines(log);
        if now.len() >= count || Instant::now() >= deadline {
            return now;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn on_wsl_or_flatpak() -> bool {
    std::fs::read_to_string("/proc/version")
        .is_ok_and(|version| version.to_lowercase().contains("microsoft"))
        || Path::new("/.flatpak-info").exists()
}

#[test]
fn stubs_are_the_only_openers_and_see_the_urls_the_scenario_expects() {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts");
    let dir = tempfile::tempdir().unwrap();
    let stubs = dir.path().join("stubs");
    let made = Command::new("bash")
        .arg(scripts.join("webview-link-opener-stubs.sh"))
        .arg(&stubs)
        .output()
        .unwrap();
    assert!(made.status.success(), "{made:?}");
    let log = stubs.join("opener.log");
    assert!(lines(&log).is_empty(), "the log starts empty");

    let path = format!(
        "{}:{}",
        stubs.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    // SAFETY: this is the only test in this binary, so no other thread is
    // reading or writing the environment.
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("BROWSER", stubs.join("browser-stub"));
    }

    // 1. The two URLs that may open arrive bare, and nothing else runs.
    //    One at a time: `webbrowser` starts the stub in the background, so two
    //    back-to-back opens can land in either order. The scenario's clicks are
    //    a settle window apart, so its log is in click order.
    webbrowser::open("https://example.com/x").unwrap();
    assert_eq!(wait_for_lines(&log, 1), ["https://example.com/x"]);
    webbrowser::open("mailto:someone@example.com").unwrap();
    assert_eq!(
        wait_for_lines(&log, 2),
        ["https://example.com/x", "mailto:someone@example.com"]
    );

    // 2. What the leak looked like: a relative path, as a file URL under the
    //    process's working directory. The log would show it.
    webbrowser::open("other.md").unwrap();
    let all = wait_for_lines(&log, 3);
    assert_eq!(all.len(), 3, "{all:?}");
    assert!(
        all[2].starts_with("file:///") && all[2].ends_with("/other.md"),
        "{all:?}"
    );

    // 3. With $BROWSER gone, the fallbacks are the shims, never a real opener.
    //    (Skipped on WSL and Flatpak, where `webbrowser` reaches Windows or the
    //    portal instead of a shimmed name.)
    if !on_wsl_or_flatpak() {
        // SAFETY: as above.
        unsafe { std::env::remove_var("BROWSER") };
        let before = lines(&log).len();
        let _ = webbrowser::open("https://example.com/y");
        let after = wait_for_lines(&log, before + 1);
        let fallbacks = &after[before..];
        assert!(
            !fallbacks.is_empty(),
            "no fallback opener ran at all: {after:?}"
        );
        for line in fallbacks {
            assert!(
                line.starts_with("UNEXPECTED-OPENER "),
                "a real opener ran, or a stub logged wrongly: {line:?}"
            );
        }
    }
}
