//! RFC-046 slice 2, part A, scenario `paste_probe`: what does the WebView do with a
//! paste, before any product code assumes an answer?
//!
//! Slice 2's paste handler rests on three things nobody has seen in a WebView:
//! that a real Ctrl+V reaches a `paste` listener on the editor with **both** the
//! `text/html` and `text/plain` flavours; what Ctrl+Shift+V does; and, only if the
//! first fails, whether a script can build a paste event that reaches a handler.
//! This scenario looks, and **reports; it asserts no product behaviour**. It
//! passes once it has observed and reported, whatever the answers are. It fails
//! only if it cannot observe at all (the editor never opens or takes focus).
//!
//! The clipboard is owned by `scripts/webview-clipboard-owner.py`, started here
//! (like `xdotool`, it inherits the Xvfb display). CI passes its path in
//! `BEKOEDIT_PASTE_PROBE_CLIPBOARD_OWNER`. Keys are real XTEST input. The page
//! side is `paste_probe.js`; the reading of what it recorded is `paste_report.rs`.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::paste_report::{Case, Constructed, Helper, Observation, Taken, answers};
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "paste_probe";
const PROBE_JS: &str = include_str!("paste_probe.js");
const CLIPBOARD_OWNER_ENV: &str = "BEKOEDIT_PASTE_PROBE_CLIPBOARD_OWNER";

/// What a browser puts on the clipboard: a heading, a paragraph with bold, a list.
const HTML_FLAVOUR: &str =
    "<h1>Probe heading</h1><p>Some <b>bold</b> text.</p><ul><li>one</li><li>two</li></ul>";
const PLAIN_FLAVOUR: &str = "Probe heading\nSome bold text.\none\ntwo";

/// After a key chord, how long the page is given to show what it does.
const SETTLE: Duration = Duration::from_millis(1500);
const OWNER_READY_DEADLINE: Duration = Duration::from_secs(10);

/// Starts the clipboard owner and waits for it to say it holds the selection.
/// Never an error: a helper that did not start is an answer the report gives.
/// `script` is what CI put in `BEKOEDIT_PASTE_PROBE_CLIPBOARD_OWNER`, if anything.
async fn start_clipboard_owner(
    dir: &Path,
    script: Option<std::ffi::OsString>,
    ready_deadline: Duration,
) -> (Option<tokio::process::Child>, Helper) {
    let Some(script) = script else {
        return (
            None,
            Helper {
                ready: false,
                note: format!("{CLIPBOARD_OWNER_ENV} is not set"),
            },
        );
    };
    let ready_file = dir.join("ready");
    let stderr_file = dir.join("owner.stderr");
    let stderr = match std::fs::File::create(&stderr_file) {
        Ok(file) => Stdio::from(file),
        Err(_) => Stdio::null(),
    };
    let spawned = tokio::process::Command::new("python3")
        .arg(&script)
        .arg(HTML_FLAVOUR)
        .arg(PLAIN_FLAVOUR)
        .arg(&ready_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            return (
                None,
                Helper {
                    ready: false,
                    note: format!("could not start python3: {error}"),
                },
            );
        }
    };
    let started = tokio::time::Instant::now();
    while started.elapsed() < ready_deadline {
        if ready_file.exists() {
            return (
                Some(child),
                Helper {
                    ready: true,
                    note: String::new(),
                },
            );
        }
        if let Ok(Some(status)) = child.try_wait() {
            let said = std::fs::read_to_string(&stderr_file).unwrap_or_default();
            return (
                None,
                Helper {
                    ready: false,
                    note: format!("the helper exited with {status}; stderr: {}", said.trim()),
                },
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let said = std::fs::read_to_string(&stderr_file).unwrap_or_default();
    (
        Some(child),
        Helper {
            ready: false,
            note: format!(
                "not ready within {OWNER_READY_DEADLINE:?}; stderr: {}",
                said.trim()
            ),
        },
    )
}

async fn take() -> Result<Taken, String> {
    let value = dom::value_of("window.__pasteProbe.take()").await?;
    serde_json::from_value(value).map_err(|error| format!("{NAME}: unreadable recording: {error}"))
}

/// Puts the caret in the editor, sends a real key chord, waits, and returns what
/// the page recorded meanwhile and the editor's text either side.
async fn press(desktop: &DesktopContext, keys: &str) -> Result<Case, String> {
    click_via_xtest(desktop, ".cm-content", None, 0).await?;
    let before = take().await?;
    run_xdotool(&["key", "--clearmodifiers", keys]).await?;
    tokio::time::sleep(SETTLE).await;
    let after = take().await?;
    Ok(Case {
        events: after.events,
        doc_before: before.doc,
        doc_after: after.doc,
    })
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
    wait_until(
        NAME,
        "the editor to open the file and take focus",
        || async { dom::editor_focused().await },
    )
    .await?;
    dom::run_script::<serde_json::Value>(PROBE_JS).await?;
    dom::value_of("window.__pasteProbe.install()").await?;

    let scratch =
        tempfile::tempdir().map_err(|error| format!("{NAME}: no scratch directory: {error}"))?;
    // Held until the end: dropping it kills the helper and gives up the selection.
    let (_owner, helper) = start_clipboard_owner(
        scratch.path(),
        std::env::var_os(CLIPBOARD_OWNER_ENV),
        OWNER_READY_DEADLINE,
    )
    .await;
    println!(
        "  {NAME}: clipboard owner ready: {} {}",
        helper.ready, helper.note
    );

    let ctrl_v = press(desktop, "ctrl+v").await?;
    let chord = press(desktop, "ctrl+shift+v").await?;

    // Asked whatever (1) showed: one CI run is expensive, and the answer costs a
    // moment. The report says when it was not needed.
    let constructed_value = dom::value_of("window.__pasteProbe.construct()").await?;
    let constructed: Constructed = serde_json::from_value(constructed_value)
        .map_err(|error| format!("{NAME}: unreadable constructed-event result: {error}"))?;

    Ok(answers(&Observation {
        helper,
        ctrl_v,
        chord,
        constructed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A stand-in for the helper: a Python script that does what the test says.
    /// It touches no display and no GTK, unlike the real one.
    fn script(dir: &Path, body: &str) -> std::ffi::OsString {
        let path = dir.join("stand-in.py");
        std::fs::write(&path, body).unwrap();
        path.into_os_string()
    }

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn a_helper_that_writes_its_ready_file_is_ready_and_is_handed_back_alive() {
        let dir = tempfile::tempdir().unwrap();
        let body = "import sys, time\nopen(sys.argv[3], 'w').write('READY')\ntime.sleep(30)\n";
        let (child, helper) = run(start_clipboard_owner(
            dir.path(),
            Some(script(dir.path(), body)),
            SECOND * 5,
        ));
        assert!(helper.ready, "{}", helper.note);
        assert!(helper.note.is_empty());
        let mut child = child.expect("the running helper is returned, so it can be kept alive");
        assert!(child.try_wait().unwrap().is_none(), "still running");
        drop(child); // kill_on_drop
    }

    #[test]
    fn the_helper_receives_both_flavours_and_the_ready_path_as_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("args.txt");
        let body = format!(
            "import sys\nopen({record:?}, 'w').write(repr(sys.argv[1:4]))\nopen(sys.argv[3], 'w').write('READY')\n"
        );
        let (_child, helper) = run(start_clipboard_owner(
            dir.path(),
            Some(script(dir.path(), &body)),
            SECOND * 5,
        ));
        assert!(helper.ready, "{}", helper.note);
        let args = std::fs::read_to_string(&record).unwrap();
        assert!(
            args.contains("Probe heading") && args.contains("<h1>Probe heading</h1>"),
            "{args}"
        );
        assert!(args.contains("two"), "{args}");
    }

    #[test]
    fn a_helper_that_exits_early_reports_its_status_and_what_it_said() {
        let dir = tempfile::tempdir().unwrap();
        let body = "import sys\nsys.stderr.write('ModuleNotFoundError: gi')\nsys.exit(3)\n";
        let (child, helper) = run(start_clipboard_owner(
            dir.path(),
            Some(script(dir.path(), body)),
            SECOND * 5,
        ));
        assert!(!helper.ready);
        assert!(child.is_none());
        assert!(
            helper.note.contains("exited") && helper.note.contains("ModuleNotFoundError: gi"),
            "{}",
            helper.note
        );
    }

    #[test]
    fn a_helper_that_never_says_ready_times_out_and_is_still_handed_back() {
        let dir = tempfile::tempdir().unwrap();
        let body = "import time\ntime.sleep(30)\n";
        let started = std::time::Instant::now();
        let (child, helper) = run(start_clipboard_owner(
            dir.path(),
            Some(script(dir.path(), body)),
            Duration::from_millis(300),
        ));
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(
            !helper.ready && helper.note.contains("not ready within"),
            "{}",
            helper.note
        );
        assert!(child.is_some(), "kept so that it is killed when dropped");
    }

    #[test]
    fn no_configured_helper_is_said_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let (child, helper) = run(start_clipboard_owner(dir.path(), None, SECOND));
        assert!(child.is_none() && !helper.ready);
        assert!(
            helper.note.contains("BEKOEDIT_PASTE_PROBE_CLIPBOARD_OWNER"),
            "{}",
            helper.note
        );
    }
}
