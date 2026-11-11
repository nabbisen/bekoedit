//! RFC-050 slice 1, level 3, the app's side of `scripts/a11y-observer.py`.
//!
//! The observer is a separate process that reads bekoedit's AT-SPI tree and acts
//! only through AT-SPI actions: no clicks, no keystrokes. This scenario does not
//! touch the UI either. It
//!
//!  1. writes the expectations file the observer reads: every accessible name the
//!     observer needs, looked up in Japanese through `i18n.rs`'s own `tr`, so that
//!     no expected string is typed anywhere else (the run is in Japanese,
//!     RFC-050 §5.1: non-ASCII text through the whole path);
//!  2. waits for `note.md` to change on disk, and asserts its exact bytes: the
//!     marker, then the original. This is the proof that a file was edited and
//!     saved through accessibility actions alone (autosave is pushed out to a day
//!     in a release-checks run, so only the Save action can have saved it);
//!  3. waits for the observer to finish (it writes a marker file), so the window
//!     stays up for its remaining steps.
//!
//! It is not in `ci.yml`'s scenario loop: it needs the observer, a D-Bus session
//! and the AT-SPI bus, which `scripts/a11y-tree-run.sh` provides.

use std::path::{Path, PathBuf};
use std::time::Duration;

use dioxus::desktop::DesktopContext;

use super::ReleaseChecksTerminal;
use super::seed::{EDIT_MARKER, SAVE_FILE};
use crate::i18n::{Lang, tr};

pub(super) const NAME: &str = "a11y_tree";

/// The file the observer reads, and the two it writes: its verdict on success,
/// and the reason on an abort.
pub(super) const EXPECTED_FILE: &str = "a11y-expected.json";
pub(super) const DONE_FILE: &str = "a11y-observer-done";
pub(super) const ABORT_FILE: &str = "a11y-observer-abort";

/// A second Markdown file, so the explorer has more than one row.
pub(super) const SECOND_FILE: &str = "other.md";

/// `note.md`, as seeded: one block of every kind Form Mode shows as a field. The
/// headless markup checks (level 2) render this same text.
pub(crate) const A11Y_NOTE: &str = "# Title\n\nA paragraph.\n\n> A quote.\n\n- [ ] a task\n- [x] a done task\n- plain\n\n```rust\nfn main() {}\n```\n\n---\n\n![alt text](picture.png)\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<div>raw html</div>\n";

/// How long each wait lasts. The observer's steps run in between.
const WAIT: Duration = Duration::from_secs(240);
const POLL: Duration = Duration::from_millis(200);

/// Every name the observer needs, by `i18n.rs` key. The observer lists the keys it
/// requires (`REQUIRED_KEYS` in `scripts/a11y_tree.py`) and a test compares the
/// two lists, so they cannot drift.
pub(super) const A11Y_KEYS: &[&str] = &[
    "app.title",
    "explorer.label",
    "explorer.tree_label",
    "explorer.new_file",
    "mode.text",
    "mode.form",
    "mode.preview",
    "editor.mode_label",
    "editor.save",
    "menu.app",
    "settings.title",
    "settings.language",
    "settings.close",
    "block.kind.heading",
    "block.kind.paragraph",
    "block.kind.table",
    "form.undo",
    "form.redo",
    "save.clean",
    "save.dirty",
    "save.scheduled",
    "save.saving",
    "save.saved",
    "save.failed",
    "save.conflict",
    "toast.dismiss",
];

/// The expectations file's content for `lang`.
pub(super) fn expectations(lang: Lang) -> serde_json::Value {
    let names: serde_json::Map<String, serde_json::Value> = A11Y_KEYS
        .iter()
        .map(|key| {
            (
                (*key).to_string(),
                serde_json::Value::String(tr(lang, key).to_string()),
            )
        })
        .collect();
    serde_json::json!({
        "version": 1,
        "language": if lang == Lang::Ja { "ja" } else { "en" },
        "marker": EDIT_MARKER,
        "files": [SAVE_FILE, SECOND_FILE],
        "document": A11Y_NOTE,
        "names": names,
    })
}

/// Written whole and then renamed into place, so the observer never reads half.
fn write_expectations(root: &Path) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&expectations(Lang::Ja))
        .map_err(|e| format!("{NAME}: {e}"))?;
    let tmp = root.join(format!("{EXPECTED_FILE}.tmp"));
    std::fs::write(&tmp, text)
        .map_err(|e| format!("{NAME}: cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, root.join(EXPECTED_FILE))
        .map_err(|e| format!("{NAME}: cannot publish the expectations: {e}"))
}

/// What a wait for the saved bytes ended with.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SavedBytes {
    /// The marker, then the original, exactly.
    Exact,
    /// The file changed, but not to that: quoted for the failure message.
    Different(String),
}

/// The check on the bytes, pure so that it is tested without a window.
pub(super) fn judge_saved_bytes(original: &[u8], saved: &[u8]) -> SavedBytes {
    let mut expected = EDIT_MARKER.as_bytes().to_vec();
    expected.extend_from_slice(original);
    if saved == expected {
        SavedBytes::Exact
    } else {
        SavedBytes::Different(format!(
            "expected {:?} followed by the original ({} bytes), got {:?} ({} bytes)",
            EDIT_MARKER,
            original.len(),
            String::from_utf8_lossy(&saved[..saved.len().min(80)]),
            saved.len()
        ))
    }
}

/// The observer stops at its first anomaly and says why; the run then fails with
/// that reason instead of waiting out a deadline for something that cannot come.
fn observer_abort(root: &Path) -> Result<(), String> {
    let path = root.join(ABORT_FILE);
    if !path.exists() {
        return Ok(());
    }
    let reason = std::fs::read_to_string(&path).unwrap_or_default();
    Err(format!(
        "{NAME}: the observer aborted: {}",
        reason.lines().next().unwrap_or("(no reason written)")
    ))
}

async fn wait_for<T>(
    what: &str,
    mut check: impl FnMut() -> Result<Option<T>, String>,
) -> Result<T, String> {
    let started = tokio::time::Instant::now();
    loop {
        if let Some(found) = check()? {
            return Ok(found);
        }
        if started.elapsed() >= WAIT {
            return Err(format!(
                "{NAME}: timed out after {}s waiting for {what}",
                WAIT.as_secs()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    _desktop: &DesktopContext,
) -> Result<Vec<String>, String> {
    let expectation = &terminal.expectation;
    let file = expectation
        .file
        .as_ref()
        .ok_or_else(|| format!("{NAME}: no file was seeded"))?;
    let root: PathBuf = expectation
        .workspace
        .parent()
        .ok_or_else(|| format!("{NAME}: the workspace has no parent"))?
        .to_path_buf();
    write_expectations(&root)?;
    println!(
        "  {NAME}: expectations written to {}",
        root.join(EXPECTED_FILE).display()
    );

    let original = expectation.original.clone();
    let saved = wait_for(
        "note.md to change on disk (the observer presses Save)",
        || {
            observer_abort(&root)?;
            let bytes = std::fs::read(file)
                .map_err(|e| format!("{NAME}: cannot read {}: {e}", file.display()))?;
            Ok((bytes != original).then_some(bytes))
        },
    )
    .await?;
    // A save is an atomic replace; read again once it has settled.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let settled =
        std::fs::read(file).map_err(|e| format!("{NAME}: cannot read {}: {e}", file.display()))?;
    let bytes = if settled.is_empty() { saved } else { settled };
    if let SavedBytes::Different(detail) = judge_saved_bytes(&original, &bytes) {
        return Err(format!("{NAME}: note.md changed, but {detail}"));
    }

    let verdict = wait_for("the observer to finish", || {
        observer_abort(&root)?;
        Ok(root.join(DONE_FILE).exists().then_some(()))
    })
    .await;
    verdict?;

    Ok(vec![
        "note.md holds the marker followed by the original, byte for byte, after an edit and a Save made through accessibility actions alone".to_string(),
        "the observer finished its checks of the tree".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_the_observer_needs_has_text_in_japanese_and_in_english() {
        for lang in [Lang::Ja, Lang::En] {
            for key in A11Y_KEYS {
                assert!(!tr(lang, key).is_empty(), "{lang:?}: {key} has no text");
            }
        }
        assert_ne!(tr(Lang::Ja, "editor.save"), tr(Lang::En, "editor.save"));
    }

    #[test]
    fn the_expectations_carry_japanese_names_from_the_apps_own_lookup() {
        let value = expectations(Lang::Ja);
        assert_eq!(value["language"], "ja");
        assert_eq!(value["names"]["editor.save"], tr(Lang::Ja, "editor.save"));
        assert_eq!(value["names"].as_object().unwrap().len(), A11Y_KEYS.len());
        assert_eq!(value["marker"], EDIT_MARKER);
        assert_eq!(value["files"], serde_json::json!(["note.md", "other.md"]));
    }

    #[test]
    fn the_saved_bytes_are_judged_exactly() {
        let original = b"# Title\r\nrest\n";
        let mut exact = EDIT_MARKER.as_bytes().to_vec();
        exact.extend_from_slice(original);
        assert_eq!(judge_saved_bytes(original, &exact), SavedBytes::Exact);
        for wrong in [
            original.to_vec(),
            [original.as_slice(), EDIT_MARKER.as_bytes()].concat(),
            exact[..exact.len() - 1].to_vec(),
            exact
                .iter()
                .map(|b| if *b == b'\r' { b'\n' } else { *b })
                .collect(),
        ] {
            assert!(
                matches!(
                    judge_saved_bytes(original, &wrong),
                    SavedBytes::Different(_)
                ),
                "{wrong:?}"
            );
        }
    }
}
