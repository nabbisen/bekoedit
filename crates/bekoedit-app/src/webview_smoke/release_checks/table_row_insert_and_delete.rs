//! RFC-048 slice 3 §2.4, scenario `table_row_insert_and_delete`: the row
//! grid's own actions menu, end to end -- a real click opens a row's
//! menu, a real click on "Insert row below" inserts a new row, real
//! keystrokes type into it, a real Ctrl+S saves it (autosave is off in
//! this run mode, task 054), then the same row is removed through the
//! menu's "Delete row" and a second Ctrl+S must bring back the exact
//! original bytes.
//!
//! The seeded table (`seed::original_table_note`) has two data rows, so
//! there is a row above and below the one this scenario inserts next to
//! -- the inserted row is never the table's first or last.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::bytes::{check_bytes_unchanged, check_saved_bytes};
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "table_row_insert_and_delete";

/// Index, in document order among every `.table-cell-input`, of the new
/// row's first cell once it exists: two header cells, row 1's two
/// cells, then the new row's own first cell.
const NEW_ROW_FIRST_CELL_INDEX: usize = 4;

/// Typed into the new row's first cell. Distinct from every digit the
/// seeded table already holds, so `check_saved_bytes` can find it
/// unambiguously.
const TYPED_TEXT: &str = "ZQ7";

/// The exact line the typed text produces once it lands in the new
/// (otherwise still-empty) row -- computed once, directly from the real
/// resolver, not guessed: a byte mismatch here would be a change to
/// that rule, not a regression in this scenario. The RFC-048 slice 3
/// review (2026-10-03 §3) changed this: a cell whose content is only
/// whitespace, two characters or more (every empty row this slice
/// creates is exactly this shape), now takes a typed edit after the
/// *first* whitespace character, so `| ZQ7 |`, not `|  ZQ7|`.
const INSERTED_LINE: &str = "| ZQ7 |  |\n";

/// A save settling, past the write itself -- the same margin every
/// other save-waiting scenario uses.
const SAVE_DEADLINE: Duration = Duration::from_secs(10);

async fn wait_for_save(file: &std::path::Path, before: &[u8]) -> Result<Vec<u8>, String> {
    let started = tokio::time::Instant::now();
    loop {
        let saved = std::fs::read(file)
            .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
        if saved != before {
            tokio::time::sleep(Duration::from_millis(300)).await;
            return std::fs::read(file)
                .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()));
        }
        if started.elapsed() >= SAVE_DEADLINE {
            return Ok(saved);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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
        "Form Mode to show the seeded table's two rows",
        || async { Ok(dom::table_row_actions_button_count().await? == 2) },
    )
    .await?;

    // The real act under test: a click opens row 1's own menu, a click
    // on "Insert row below" inserts the new row -- never a synthetic
    // dispatch of `FormBlockEdit::InsertTableRow` from the scenario
    // itself, which would prove nothing about the grid's own UI.
    click_via_xtest(desktop, ".table-row-actions-btn", None, 0).await?;
    wait_until(NAME, "row 1's menu to open", || async {
        dom::table_row_menu_open().await
    })
    .await?;
    click_via_xtest(
        desktop,
        ".table-row-actions-menu .dropdown-item",
        Some("Insert row below"),
        0,
    )
    .await?;
    wait_until(NAME, "the new row to appear", || async {
        Ok(dom::table_row_actions_button_count().await? == 3)
    })
    .await?;

    // A real click into the new row's own first cell, then real
    // keystrokes -- never the field's `.value` set from script.
    click_via_xtest(desktop, ".table-cell-input", None, NEW_ROW_FIRST_CELL_INDEX).await?;
    run_xdotool(&["type", "--clearmodifiers", TYPED_TEXT]).await?;
    wait_until(NAME, "the new cell to hold the typed text", || async {
        Ok(dom::table_cell_value_at(NEW_ROW_FIRST_CELL_INDEX).await?
            == Some(TYPED_TEXT.to_string()))
    })
    .await?;

    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_insert = wait_for_save(file, original).await?;
    let insert_at = check_saved_bytes(NAME, original, &after_insert, INSERTED_LINE.as_bytes())
        .map_err(|error| format!("{NAME}: after inserting and saving: {error}"))?;

    // The row that now holds the typed text is the table's second
    // actions button (row 1, then this new row, then the original row
    // 2) -- open its menu and delete it.
    click_via_xtest(desktop, ".table-row-actions-btn", None, 1).await?;
    wait_until(NAME, "the new row's menu to open", || async {
        dom::table_row_menu_open().await
    })
    .await?;

    // Task 056 §2.3: report what the scenario saw right before the
    // Delete click -- a real click with no visible effect gave no clue
    // by itself whether an edit was even attempted.
    let before_delete_count = dom::table_row_actions_button_count().await?;
    let (active_trigger_id, open_menu_id) = dom::open_row_actions_ids().await?;
    println!(
        "  {NAME}: before the Delete row click: {before_delete_count} row-actions buttons, \
         active trigger id={active_trigger_id:?}, open menu id={open_menu_id:?}"
    );

    click_via_xtest(
        desktop,
        ".table-row-actions-menu .dropdown-item",
        Some("Delete row"),
        0,
    )
    .await?;
    if let Err(error) = wait_until(NAME, "the inserted row to be gone", || async {
        Ok(dom::table_row_actions_button_count().await? == 2)
    })
    .await
    {
        // Task 056 §2.3: the assertion itself is unchanged -- this only
        // adds what the scenario saw after the timeout, for the merge
        // report to read against the trace above and `form_trace`'s own
        // log.
        let after_delete_count = dom::table_row_actions_button_count().await;
        let toast_seen = dom::toast_present().await;
        return Err(format!(
            "{error}; after the timeout: row-actions buttons={after_delete_count:?}, \
             toast present={toast_seen:?}"
        ));
    }

    run_xdotool(&["key", "--clearmodifiers", "ctrl+s"]).await?;
    let after_delete = wait_for_save(file, &after_insert).await?;
    check_bytes_unchanged(NAME, original, &after_delete)
        .map_err(|error| format!("{NAME}: after deleting and saving: {error}"))?;

    Ok(vec![
        format!("the inserted line landed at byte {insert_at}, with every other byte unchanged"),
        "deleting it and saving again restored the exact original bytes".to_string(),
    ])
}
