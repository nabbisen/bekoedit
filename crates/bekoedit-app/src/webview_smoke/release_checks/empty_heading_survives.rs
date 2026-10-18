//! Task 064, scenario `empty_heading_survives`: an empty heading (`# `) never
//! crashes bekoedit, on the three paths that used to panic in
//! `MarkdownIndex::build`. Every click is a real XTEST click through task
//! 058's guard, every key a real keystroke, and every save a real Ctrl+S
//! (autosave is off in this run mode, task 054).
//!
//! 1. **Text Mode sync:** type `# ` on a new line, wait past the editor's
//!    100 ms change debounce until the app has applied it (the document shows
//!    its unsaved dot), then keep typing `Title`, save, and compare the bytes.
//! 2. **Opening a file** that holds `# \n`, to Form Mode and back: the bytes
//!    are unchanged, in the editor and on disk.
//! 3. **Form Mode:** clear the heading's field, commit it with a real Ctrl+S,
//!    and compare the exact bytes of the empty heading it leaves.
//!
//! The app answering a read after each step is itself the check that it did
//! not crash: a panic takes the window, and the read fails.

use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::{EMPTY_HEADING_FILE, SAVE_FILE, original_empty_heading_note};
use super::table_row_insert_and_delete::wait_for_save;

pub(super) const NAME: &str = "empty_heading_survives";

/// The seeded `note.md` is `body\n` (`seed::original_body_note`). The cursor
/// goes to its end, on a new empty line, and `# ` then `Title` are typed.
const AFTER_TYPING: &str = "body\n# Title";
/// The heading's field cleared: the hashes and the one space stay.
const AFTER_CLEARING: &str = "body\n# ";

async fn key(keys: &str) -> Result<(), String> {
    run_xdotool(&["key", "--clearmodifiers", keys]).await
}

/// The editor's text, or `None` while there is no editor view yet.
async fn editor_text_now() -> Result<Option<String>, String> {
    match dom::value_of("window.__bk?._view?.state.doc.toString() ?? null").await? {
        serde_json::Value::String(text) => Ok(Some(text)),
        serde_json::Value::Null => Ok(None),
        other => Err(format!("{NAME}: unexpected editor text value ({other})")),
    }
}

async fn editor_text_is(expected: &str) -> Result<bool, String> {
    Ok(editor_text_now().await?.as_deref() == Some(expected))
}

/// Task 065: what a wait for the editor's text says when the text is not yet
/// what it should be, so a timeout names this condition and quotes the text
/// instead of leaving it to be guessed from a bare "timed out".
fn text_mismatch(expected: &str, actual: Option<&str>) -> String {
    match actual {
        Some(actual) => format!("the editor holds {actual:?}, not the seeded text {expected:?}"),
        None => format!("the editor has no text (null), not the seeded text {expected:?}"),
    }
}

/// Waits for the editor's text alone, with no condition on focus. The last
/// mismatch is the timeout's "last error", so it is quoted.
async fn wait_editor_text(what: &str, expected: &str) -> Result<(), String> {
    wait_until(NAME, what, || async {
        let actual = editor_text_now().await?;
        if actual.as_deref() == Some(expected) {
            Ok(true)
        } else {
            Err(text_mismatch(expected, actual.as_deref()))
        }
    })
    .await
}

async fn heading_values() -> Result<Vec<String>, String> {
    let value = dom::value_of(
        "[...document.querySelectorAll('.form-mode .heading-input')].map(el => el.value)",
    )
    .await?;
    serde_json::from_value(value).map_err(|error| format!("{NAME}: {error}"))
}

async fn wait_heading_fields(expected: &[&str]) -> Result<(), String> {
    wait_until(NAME, "Form Mode to show the heading field", || async {
        Ok(heading_values().await? == expected)
    })
    .await
}

async fn open_in_tree(desktop: &DesktopContext, file: &str) -> Result<(), String> {
    click_via_xtest(desktop, ".tree-row.tree-file", Some(file), 0).await
}

fn bytes_error(what: &str, expected: &str, got: &[u8]) -> String {
    format!(
        "{NAME}: {what}: expected {expected:?}, got {:?}",
        String::from_utf8_lossy(got)
    )
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
    let empty_file = expectation.workspace.join(EMPTY_HEADING_FILE);
    let original = &expectation.original;
    let empty_original = original_empty_heading_note();

    wait_until(
        NAME,
        "the workspace tree to show the seeded files",
        || async { Ok(dom::snapshot().await?.tree_rows >= 2) },
    )
    .await?;
    activate_window(desktop);

    // 1. Text Mode: `# ` on a new line, synced, then `Title`.
    open_in_tree(desktop, SAVE_FILE).await?;
    wait_until(
        NAME,
        "the editor to open note.md and take focus",
        || async { Ok(dom::editor_focused().await? && editor_text_is("body\n").await?) },
    )
    .await?;
    key("ctrl+End").await?;
    run_xdotool(&["type", "--clearmodifiers", "--delay", "60", "# "]).await?;
    wait_until(NAME, "the app to apply the synced `# `", || async {
        dom::document_dirty().await
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    if !editor_text_is("body\n# ").await? {
        return Err(format!("{NAME}: the editor lost the typed `# `"));
    }
    run_xdotool(&["type", "--clearmodifiers", "--delay", "60", "Title"]).await?;
    key("ctrl+s").await?;
    let typed = wait_for_save(file, original).await?;
    if typed != AFTER_TYPING.as_bytes() {
        return Err(bytes_error(
            "after typing `# ` then `Title`",
            AFTER_TYPING,
            &typed,
        ));
    }

    // 2. A file that opens with an empty heading, to Form Mode and back.
    open_in_tree(desktop, EMPTY_HEADING_FILE).await?;
    let empty_text = String::from_utf8_lossy(&empty_original).into_owned();
    wait_until(NAME, "empty.md to open in the editor", || async {
        Ok(dom::editor_focused().await? && editor_text_is(&empty_text).await?)
    })
    .await?;
    key("ctrl+2").await?;
    wait_heading_fields(&[""]).await?;
    key("ctrl+1").await?;
    // The text only. Whether the editor takes focus after Ctrl+2 then Ctrl+1
    // with nothing focused in Form Mode is a separate question (task 065); it
    // is recorded below, not asserted.
    wait_editor_text("Text Mode to return with the same text", &empty_text).await?;
    let focused_after_return = dom::editor_focused().await?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    let on_disk = std::fs::read(&empty_file)
        .map_err(|error| format!("{NAME}: cannot read {}: {error}", empty_file.display()))?;
    if on_disk != empty_original || dom::document_dirty().await? {
        return Err(bytes_error(
            "empty.md after Form Mode and back",
            &empty_text,
            &on_disk,
        ));
    }

    // 3. Form Mode: clear the heading's field and commit it.
    open_in_tree(desktop, SAVE_FILE).await?;
    wait_until(NAME, "note.md to open again", || async {
        Ok(dom::editor_focused().await? && editor_text_is(AFTER_TYPING).await?)
    })
    .await?;
    key("ctrl+2").await?;
    wait_heading_fields(&["Title"]).await?;
    click_via_xtest(desktop, ".form-mode .heading-input", None, 0).await?;
    key("ctrl+a").await?;
    key("BackSpace").await?;
    key("ctrl+s").await?;
    let cleared = wait_for_save(file, &typed).await?;
    if cleared != AFTER_CLEARING.as_bytes() {
        return Err(bytes_error(
            "after clearing the heading",
            AFTER_CLEARING,
            &cleared,
        ));
    }
    wait_heading_fields(&[""]).await?;

    Ok(vec![
        format!(
            "after Ctrl+2 then Ctrl+1 with nothing focused, the editor is focused: \
             {focused_after_return}"
        ),
        "typing `# ` in Text Mode synced without a crash, and `Title` saved byte-exact".to_string(),
        "a file holding `# ` opened, went to Form Mode and back, with its bytes unchanged"
            .to_string(),
        "clearing a heading in Form Mode left the empty heading, saved byte-exact".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants the scenario compares against must follow from the seed
    /// and the typing, or the scenario would probe the wrong bytes.
    #[test]
    fn the_expected_bytes_follow_from_the_seed_and_the_typing() {
        assert_eq!(format!("{}{}", "body\n", "# Title"), AFTER_TYPING);
        assert_eq!(AFTER_TYPING.replace("Title", ""), AFTER_CLEARING);
        assert_ne!(AFTER_TYPING, AFTER_CLEARING);
    }

    /// The timeout of the text wait names the condition and quotes the text.
    #[test]
    fn a_text_mismatch_quotes_the_actual_text_or_says_null() {
        assert_eq!(
            text_mismatch("# \n\ntext\n", Some("# \ntext\n")),
            "the editor holds \"# \\ntext\\n\", not the seeded text \"# \\n\\ntext\\n\""
        );
        assert_eq!(
            text_mismatch("# \n", None),
            "the editor has no text (null), not the seeded text \"# \\n\""
        );
    }
}
