//! Task 027, `mode_switch_preserves_bytes`: open a mixed-line-ending file in
//! Text Mode with a real click, make **no edit**, switch to Preview with a real
//! click, wait past the autosave debounce, and read the bytes on disk. They
//! must equal the seeded bytes, and the document must never have been dirty.

use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::state::AUTOSAVE_DEBOUNCE_MS;
use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest};

use super::ReleaseChecksTerminal;
use super::bytes::check_bytes_unchanged;
use super::dom;
use super::save::{POLL_INTERVAL, wait_until};
use super::seed::SAVE_FILE;

const NAME: &str = "mode_switch_preserves_bytes";
/// Past the autosave debounce, the app's 500 ms autosave tick, and margin.
const AUTOSAVE_WINDOW: Duration = Duration::from_millis(AUTOSAVE_DEBOUNCE_MS + 3000);

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

    wait_until(
        NAME,
        "the workspace tree to show the seeded file",
        || async { Ok(dom::snapshot().await?.tree_rows >= 2) },
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

    click_via_xtest(
        desktop,
        r#"[data-source-focus-launch="mode-preview"]"#,
        None,
        0,
    )
    .await?;
    wait_until(NAME, "the Preview tab to be selected", || async {
        dom::preview_selected().await
    })
    .await?;

    // Watch the document across the whole autosave window: it must never turn
    // dirty, not merely be clean at the end (an autosave would clean it again).
    let started = tokio::time::Instant::now();
    let mut ever_dirty = false;
    while started.elapsed() < AUTOSAVE_WINDOW {
        if let Ok(app) = state.try_peek() {
            ever_dirty |= app.session.as_ref().is_some_and(|session| session.dirty);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let saved = std::fs::read(file)
        .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
    check_bytes_unchanged(NAME, &expectation.original, &saved)?;
    if ever_dirty {
        return Err(format!(
            "{NAME}: the document became dirty with no edit (the bytes on disk match, but only \
             because nothing was written yet)"
        ));
    }
    Ok(vec![
        format!(
            "no edit, Text to Preview, {:?} of autosave window: the file's {} bytes are identical",
            AUTOSAVE_WINDOW,
            saved.len()
        ),
        "the document never turned dirty".to_string(),
    ])
}
