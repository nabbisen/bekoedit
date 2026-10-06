//! RFC-048 slice 5 §2.5, scenario `table_keyboard_and_tidy`: the grid's
//! keyboard and its Tidy action, end to end. Every click is a real XTEST click
//! through task 058's guard, and every save a real Ctrl+S (autosave is off in
//! this run mode, task 054).
//!
//! The seeded table is ragged and holds Japanese text, so Tidy has real work
//! to do by display width, not by bytes.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;
use super::table_row_insert_and_delete::wait_for_save;

pub(super) const NAME: &str = "table_keyboard_and_tidy";

/// Cell inputs in document order: two header cells, row 1's two, row 2's two.
const ROW2_COL0: usize = 4;
const ROW2_COL1: usize = 5;

/// After typing `30` into row 2's second cell, Shift+Tab back, and retyping
/// `Bob` into its first: both typed cells are committed, with no loss.
const AFTER_TYPING: &str = "| 名前 | 年齢 |\n|---|---|\n| 太郎 | 20 |\n| Bob | 30 |\n";
/// Tidied: each column as wide as its widest cell by display width (名前 is
/// 4 columns), delimiter cells as long as their column.
const AFTER_TIDY: &str = "| 名前 | 年齢 |\n| ---- | ---- |\n| 太郎 | 20   |\n| Bob  | 30   |\n";

async fn key(keys: &str) -> Result<(), String> {
    run_xdotool(&["key", "--clearmodifiers", keys]).await
}

async fn wait_focus(cell: usize, what: &str) -> Result<(), String> {
    wait_until(NAME, what, || async {
        Ok(dom::active_table_cell_index().await? == Some(cell))
    })
    .await
}

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    desktop: &DesktopContext,
) -> Result<Vec<String>, String> {
    let expectation = &terminal.expectation;
    let file = expectation
        .file
        .as_ref()
        .ok_or_else(|| format!("{NAME}: no file was seeded"))?;
    let original = &expectation.original;

    wait_until(
        NAME,
        "the workspace tree to show the seeded file",
        || async { Ok(dom::snapshot().await?.tree_rows >= 1) },
    )
    .await?;
    activate_window(desktop);
    click_via_xtest(desktop, ".tree-row.tree-file", Some(SAVE_FILE), 0).await?;
    wait_until(NAME, "Form Mode to show the seeded table", || async {
        Ok(dom::table_column_actions_button_count().await? == 2)
    })
    .await?;

    // 1. Tab through the row with real keystrokes, typing into each cell.
    click_via_xtest(desktop, ".table-cell-input", None, ROW2_COL0).await?;
    key("ctrl+a").await?;
    run_xdotool(&["type", "--clearmodifiers", "Bo"]).await?;
    key("Tab").await?;
    wait_focus(ROW2_COL1, "Tab to move to the next cell").await?;
    key("ctrl+a").await?;
    run_xdotool(&["type", "--clearmodifiers", "30"]).await?;
    key("shift+Tab").await?;
    wait_focus(ROW2_COL0, "Shift+Tab to move back").await?;
    key("ctrl+a").await?;
    run_xdotool(&["type", "--clearmodifiers", "Bob"]).await?;
    key("ctrl+s").await?;
    let after_typing = wait_for_save(file, original).await?;
    if after_typing != AFTER_TYPING.as_bytes() {
        return Err(format!(
            "{NAME}: after Tab, typing and Shift+Tab: expected {AFTER_TYPING:?}, got {:?}",
            String::from_utf8_lossy(&after_typing)
        ));
    }

    // 2. Tidy the ragged table, with Japanese text.
    click_via_xtest(desktop, ".table-tidy", None, 0).await?;
    key("ctrl+s").await?;
    let after_tidy = wait_for_save(file, &after_typing).await?;
    if after_tidy != AFTER_TIDY.as_bytes() {
        return Err(format!(
            "{NAME}: after Tidy: expected {AFTER_TIDY:?}, got {:?}",
            String::from_utf8_lossy(&after_tidy)
        ));
    }

    // 3. Tidy again: the document must not change, so it must not be dirty.
    wait_until(
        NAME,
        "the saved document to show no unsaved dot",
        || async { Ok(!dom::document_dirty().await?) },
    )
    .await?;
    click_via_xtest(desktop, ".table-tidy", None, 0).await?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    if dom::document_dirty().await? {
        return Err(format!(
            "{NAME}: Tidy on an already tidy table made the document dirty"
        ));
    }

    Ok(vec![
        "Tab and Shift+Tab moved between cells, and every typed cell was committed".to_string(),
        "Tidy aligned the Japanese table by display width, byte-exact".to_string(),
        "Tidy again changed nothing and left the document clean".to_string(),
    ])
}
