//! RFC-049 slice 2 §2.5, scenario `form_undo_redo`: Form Mode's Undo and Redo,
//! end to end. Every click is a real XTEST click through task 058's guard, every
//! key a real keystroke, and every save a real Ctrl+S (autosave is off in this
//! run mode, task 054).
//!
//! 1. Edit a table cell, delete a table row, save.
//! 2. Click a non-field area, then Ctrl+Z twice (focus on `body`: Form undo),
//!    save: the original bytes are back.
//! 3. Ctrl+Shift+Z twice, save: the bytes after both edits.
//! 4. In a paragraph field, Ctrl+Z undoes only the typing there, and the
//!    committed history (the buttons' state) does not change.
//! 5. One Undo by a real click on the button, save.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;
use super::table_row_insert_and_delete::wait_for_save;

pub(super) const NAME: &str = "form_undo_redo";

/// Cell inputs in document order: two header cells, then row 1's two.
const ROW1_COL0: usize = 2;

/// The seed (`seed::original_undo_note`) with row 1's first cell `1` typed
/// over by `9`, and the last row deleted.
const AFTER_BOTH: &str = "| a | b |\n|---|---|\n| 9 | 2 |\n\nabc\n";
/// The same, before the row is deleted: one Undo from `AFTER_BOTH`.
const AFTER_CELL_ONLY: &str = "| a | b |\n|---|---|\n| 9 | 2 |\n| 3 | 4 |\n\nabc\n";

async fn key(keys: &str) -> Result<(), String> {
    run_xdotool(&["key", "--clearmodifiers", keys]).await
}

async fn wait_rows(count: usize, what: &str) -> Result<(), String> {
    wait_until(NAME, what, || async {
        Ok(dom::table_row_actions_button_count().await? == count)
    })
    .await
}

async fn wait_cell(value: &str, what: &str) -> Result<(), String> {
    wait_until(NAME, what, || async {
        Ok(dom::table_cell_value_at(ROW1_COL0).await? == Some(value.to_string()))
    })
    .await
}

async fn expect_buttons(
    undo_disabled: bool,
    redo_disabled: bool,
    when: &str,
) -> Result<(), String> {
    let now = dom::form_history_disabled().await?;
    if now != (undo_disabled, redo_disabled) {
        return Err(format!(
            "{NAME}: {when}: (Undo disabled, Redo disabled) is {now:?}, expected \
             {:?}",
            (undo_disabled, redo_disabled)
        ));
    }
    Ok(())
}

async fn expect_bytes(
    file: &std::path::Path,
    before: &[u8],
    expected: &str,
    when: &str,
) -> Result<Vec<u8>, String> {
    key("ctrl+s").await?;
    let saved = wait_for_save(file, before).await?;
    if saved != expected.as_bytes() {
        return Err(format!(
            "{NAME}: {when}: expected {expected:?}, got {:?}",
            String::from_utf8_lossy(&saved)
        ));
    }
    Ok(saved)
}

/// Ctrl+Z in the focused paragraph field until the browser's own undo has
/// taken the typed suffix back off. WebKit decides how many undo steps the
/// typing was, so this presses until the value is the committed text again.
async fn undo_typing_in_the_field() -> Result<(), String> {
    for _ in 0..8 {
        if dom::paragraph_field_value_is("abc").await? {
            return Ok(());
        }
        key("ctrl+z").await?;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    if dom::paragraph_field_value_is("abc").await? {
        Ok(())
    } else {
        Err(format!(
            "{NAME}: Ctrl+Z inside the paragraph field never took the typed text back to \"abc\""
        ))
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
        "Form Mode to show the seeded table and paragraph",
        || async {
            Ok(dom::table_row_actions_button_count().await? == 2
                && dom::form_paragraph_present().await?)
        },
    )
    .await?;
    expect_buttons(true, true, "before any edit").await?;

    // 1. A cell edit (Tab commits it), then a row delete through its menu.
    click_via_xtest(desktop, ".table-cell-input", None, ROW1_COL0).await?;
    key("ctrl+a").await?;
    run_xdotool(&["type", "--clearmodifiers", "9"]).await?;
    key("Tab").await?;
    click_via_xtest(desktop, ".table-row-actions-btn", None, 1).await?;
    wait_until(NAME, "the last row's menu to open", || async {
        dom::table_row_menu_open().await
    })
    .await?;
    click_via_xtest(
        desktop,
        ".table-row-actions-menu .dropdown-item",
        Some("Delete row"),
        0,
    )
    .await?;
    wait_rows(1, "the last row to be deleted").await?;
    let after_both = expect_bytes(
        file,
        original,
        AFTER_BOTH,
        "after the cell edit and row delete",
    )
    .await?;
    expect_buttons(false, true, "after two committed edits").await?;

    // 2. Focus out of every field, then Ctrl+Z twice: Form undo, with `body`
    // focused (task 065's finding: that is "not a text field").
    click_via_xtest(desktop, ".form-history", None, 0).await?;
    key("ctrl+z").await?;
    wait_rows(2, "the first Undo to bring the row back").await?;
    key("ctrl+z").await?;
    wait_cell("1", "the second Undo to restore the cell").await?;
    expect_buttons(true, false, "after two Undos").await?;
    expect_bytes(
        file,
        &after_both,
        std::str::from_utf8(original).unwrap(),
        "after Ctrl+Z twice",
    )
    .await?;

    // 3. Ctrl+Shift+Z twice.
    key("ctrl+shift+z").await?;
    wait_cell("9", "the first Redo to re-apply the cell edit").await?;
    key("ctrl+shift+z").await?;
    wait_rows(1, "the second Redo to delete the row again").await?;
    expect_buttons(false, true, "after two Redos").await?;
    expect_bytes(file, original, AFTER_BOTH, "after Ctrl+Shift+Z twice").await?;

    // 4. Ctrl+Z inside a field is the browser's own: only the typing goes.
    click_via_xtest(desktop, ".form-mode .paragraph-input", None, 0).await?;
    key("End").await?;
    run_xdotool(&["type", "--clearmodifiers", " def"]).await?;
    wait_until(
        NAME,
        "the paragraph field to hold the typed text",
        || async { dom::paragraph_field_value_is("abc def").await },
    )
    .await?;
    undo_typing_in_the_field().await?;
    expect_buttons(false, true, "after Ctrl+Z inside a field").await?;

    // 5. One Undo by a real click on the button.
    click_via_xtest(desktop, "#form-undo", None, 0).await?;
    wait_rows(2, "the Undo button to bring the row back").await?;
    expect_bytes(
        file,
        &after_both,
        AFTER_CELL_ONLY,
        "after one Undo by the button",
    )
    .await?;

    Ok(vec![
        "a cell edit and a row delete were saved, then Ctrl+Z twice restored the exact original bytes"
            .to_string(),
        "Ctrl+Shift+Z twice brought back the bytes after both edits".to_string(),
        "Ctrl+Z inside a paragraph field undid only the typing; the history did not change"
            .to_string(),
        "the Undo button undid the row delete, byte-exact".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webview_smoke::release_checks::seed::original_undo_note;

    /// The expected bytes follow from the seed and the two edits, or the
    /// scenario would compare against the wrong text.
    #[test]
    fn the_expected_bytes_follow_from_the_seed() {
        let seed = String::from_utf8(original_undo_note()).unwrap();
        assert_eq!(seed.replacen("| 1 | 2 |", "| 9 | 2 |", 1), AFTER_CELL_ONLY);
        assert_eq!(AFTER_CELL_ONLY.replacen("| 3 | 4 |\n", "", 1), AFTER_BOTH);
        assert_ne!(seed, AFTER_BOTH);
    }
}
