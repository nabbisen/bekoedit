//! RFC-048 slice 4 §2.3, scenario `table_column_operations`: the column
//! grid's own menu, end to end. Every click is a real XTEST click through
//! task 058's guard, and every save is a real Ctrl+S (autosave is off in this
//! run mode, task 054). The seeded table is the one `table_row_insert_and_delete`
//! uses, so its own two data rows show what each column operation does to
//! every line, including the delimiter.

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::bytes::check_bytes_unchanged;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;
use super::table_row_insert_and_delete::wait_for_save;

pub(super) const NAME: &str = "table_column_operations";

/// The header cell typed into after the first insert: header cells in
/// document order, so the new column's cell is the second of three.
const NEW_HEADER_INDEX: usize = 1;

/// After inserting a column right of `a`, and typing `H` into its header
/// cell. The blank cell `  ` takes the typed text after its first space,
/// as slice 3's review settled (`ReplaceTableCell`).
const AFTER_INSERT: &str = "| a | H | b |\n|---| --- |---|\n| 1 |  | 2 |\n| 3 |  | 4 |\n";
/// Only the new column's delimiter cell changes, to a centred `:---:`.
const AFTER_CENTRE: &str = "| a | H | b |\n|---| :---: |---|\n| 1 |  | 2 |\n| 3 |  | 4 |\n";
/// `H` moved left: each cell's own bytes travel, so its alignment moves with it.
const AFTER_MOVE: &str = "| H | a | b |\n| :---: |---|---|\n|  | 1 | 2 |\n|  | 3 | 4 |\n";

async fn click_column_item(
    desktop: &DesktopContext,
    index: usize,
    item: &str,
) -> Result<(), String> {
    click_via_xtest(desktop, ".table-col-actions-btn", None, index).await?;
    wait_until(NAME, "a column menu to open", || async {
        dom::table_col_menu_open().await
    })
    .await?;
    click_via_xtest(desktop, ".table-col-menu .dropdown-item", Some(item), 0).await
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
    wait_until(
        NAME,
        "Form Mode to show the seeded table's two columns",
        || async { Ok(dom::table_column_actions_button_count().await? == 2) },
    )
    .await?;

    // 1. Insert a column right of the first, type a header into it, save.
    click_column_item(desktop, 0, "Insert column right").await?;
    wait_until(NAME, "the new column to appear", || async {
        Ok(dom::table_column_actions_button_count().await? == 3)
    })
    .await?;
    click_via_xtest(desktop, ".table-cell-input", None, NEW_HEADER_INDEX).await?;
    run_xdotool(&["type", "--clearmodifiers", "H"]).await?;
    wait_until(NAME, "the new header to hold the typed text", || async {
        Ok(dom::table_cell_value_at(NEW_HEADER_INDEX).await? == Some("H".to_string()))
    })
    .await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_insert = wait_for_save(file, original).await?;
    if after_insert != AFTER_INSERT.as_bytes() {
        return Err(format!(
            "{NAME}: after inserting a column and typing H: expected {AFTER_INSERT:?}, got {:?}",
            String::from_utf8_lossy(&after_insert)
        ));
    }

    // 2. Centre the new column. Only its delimiter cell may change.
    click_column_item(desktop, 1, "Centre").await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_centre = wait_for_save(file, &after_insert).await?;
    if after_centre != AFTER_CENTRE.as_bytes() {
        return Err(format!(
            "{NAME}: after centring the column: expected {AFTER_CENTRE:?}, got {:?}",
            String::from_utf8_lossy(&after_centre)
        ));
    }

    // 3. Move it left. The swapped bytes, with its alignment.
    click_column_item(desktop, 1, "Move left").await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_move = wait_for_save(file, &after_centre).await?;
    if after_move != AFTER_MOVE.as_bytes() {
        return Err(format!(
            "{NAME}: after moving the column left: expected {AFTER_MOVE:?}, got {:?}",
            String::from_utf8_lossy(&after_move)
        ));
    }

    // 4. Delete it. The original bytes come back.
    click_column_item(desktop, 0, "Delete column").await?;
    wait_until(NAME, "the column to be gone", || async {
        Ok(dom::table_column_actions_button_count().await? == 2)
    })
    .await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_delete = wait_for_save(file, &after_move).await?;
    check_bytes_unchanged(NAME, original, &after_delete)
        .map_err(|error| format!("{NAME}: after deleting the column and saving: {error}"))?;

    Ok(vec![
        "inserting a column and typing into its header saved the exact bytes".to_string(),
        "centring it changed only its delimiter cell".to_string(),
        "moving it left swapped its cells, delimiter included".to_string(),
        "deleting it restored the exact original bytes".to_string(),
    ])
}
