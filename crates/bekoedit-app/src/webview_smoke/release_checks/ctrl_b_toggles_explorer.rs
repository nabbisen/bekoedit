//! Task 054 §2.3/§2.4: Ctrl+B, with focus in the editor, toggles the
//! workspace explorer -- the same `ExplorerCollapsed` signal the
//! explorer's own toolbar button sets. `shortcuts.js` has sent
//! `toggle_explorer` since RFC-020, but no Rust code ever acted on it
//! until this task (found while diagnosing why no keyboard shortcut
//! reached Rust at all, task 053/054).

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "ctrl_b_toggles_explorer";

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
    wait_until(
        NAME,
        "the explorer to be visible before the first toggle",
        || async { dom::explorer_present().await },
    )
    .await?;

    // The real act under test: Ctrl+B, with focus already in the
    // editor, not the explorer itself -- the window-level listener
    // `shortcuts.js` installs must see it regardless.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+b"]).await?;
    wait_until(NAME, "the explorer to collapse after Ctrl+B", || async {
        Ok(!dom::explorer_present().await?)
    })
    .await?;

    run_xdotool(&["key", "--clearmodifiers", "ctrl+b"]).await?;
    wait_until(
        NAME,
        "the explorer to be restored after a second Ctrl+B",
        || async { dom::explorer_present().await },
    )
    .await?;

    Ok(vec![
        "Ctrl+B collapsed the explorer, and a second Ctrl+B restored it".to_string(),
    ])
}
