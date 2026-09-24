//! #8, `save_preserves_bytes`: open a mixed-line-ending file through the tree
//! with a real click, type one edit and save with a real Ctrl+S (real XTEST
//! input, as task 023's clicks are), then compare the file on disk against the
//! seeded original in Rust, on bytes.

use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::bytes::check_saved_bytes;
use super::dom;
use super::seed::{EDIT_MARKER, SAVE_FILE};

const NAME: &str = "save_preserves_bytes";
const STEP_DEADLINE: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(50);
/// After the file first differs, a further wait and a re-read: a save is an
/// atomic replace, and the comparison must be of the settled file.
const SETTLE: Duration = Duration::from_millis(800);

async fn wait_until<F, Fut>(what: &str, mut condition: F) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let started = tokio::time::Instant::now();
    let mut last_error = None;
    loop {
        match condition().await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => last_error = Some(error),
        }
        if started.elapsed() >= STEP_DEADLINE {
            return Err(format!(
                "{NAME}: timed out waiting for {what}{}",
                last_error.map_or(String::new(), |error| format!(" (last error: {error})"))
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    desktop: &DesktopContext,
    state: Signal<AppState>,
) -> Result<Vec<String>, String> {
    let expectation = &terminal.expectation;
    let file = expectation
        .file
        .as_ref()
        .ok_or_else(|| format!("{NAME}: no file was seeded"))?;

    wait_until("the workspace tree to show the seeded file", || async {
        Ok(dom::snapshot().await?.tree_rows >= 2)
    })
    .await?;
    activate_window(desktop);
    click_via_xtest(desktop, ".tree-row.tree-file", Some(SAVE_FILE), 0).await?;
    wait_until("the editor to open the file and take focus", || async {
        dom::editor_focused().await
    })
    .await?;

    // The document is at its start after Ctrl+Home, so the edit lands on the
    // first line -- a CRLF line, the case most likely to lose its ending.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+Home"]).await?;
    run_xdotool(&["type", "--clearmodifiers", "--delay", "60", EDIT_MARKER]).await?;
    wait_until("the typed edit to reach the editor", || async {
        dom::editor_contains(EDIT_MARKER).await
    })
    .await?;
    let dirty = state
        .try_peek()
        .map(|app| app.session.as_ref().map(|s| s.path.clone()))
        .ok()
        .flatten();
    println!("  save_preserves_bytes: session before save: {dirty:?}");

    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let started = tokio::time::Instant::now();
    loop {
        let saved = std::fs::read(file)
            .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
        if saved != expectation.original {
            break;
        }
        if started.elapsed() >= STEP_DEADLINE {
            return Err(format!(
                "{NAME}: the file on disk never changed after Ctrl+S (still the original {} bytes)",
                saved.len()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
    tokio::time::sleep(SETTLE).await;
    let saved = std::fs::read(file)
        .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
    let at = check_saved_bytes(&expectation.original, &saved, EDIT_MARKER.as_bytes())?;
    Ok(vec![
        format!(
            "one edit at byte {at}, saved through the app; the other {} bytes are identical",
            expectation.original.len()
        ),
        "no line ending was normalized".to_string(),
    ])
}
