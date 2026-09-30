//! RFC-046 slice 2, part B §3.6: the paste path end to end, with real XTEST
//! input and part A's GTK 4 clipboard owner -- the same mechanism part A used
//! to answer the gating questions, now driving the product handler itself.
//!
//! Two checks, one seeded file, in this order:
//! - **Ctrl+Shift+V** must paste the plain flavour, unconverted (§3.5). Undone
//!   before the next check, so it leaves the document as it found it.
//! - **A real Ctrl+V** of HTML with a heading, a list, bold and a table must
//!   convert and land in the editor; saving it is then checked on bytes, like
//!   task 026's other scenarios. The full HTML the page received is logged
//!   (§3.0: WebKitGTK rewrites it before the page sees it, so this is the
//!   HTML `mdka` actually converts, not what the owner served) and whether
//!   heading, list, bold and table structure survived is reported -- **not**
//!   asserted, since part A already found that computed inline styles are
//!   exactly the shape `mdka` can lose bold in (RFC-046 §6.2 item 4). Turning
//!   the logged HTML into a `webkitgtk-…` corpus fixture, with hand-typed
//!   expected Markdown, is the next step once this run's log has been read.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use dioxus::desktop::DesktopContext;

use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest, run_xdotool};

use super::ReleaseChecksTerminal;
use super::dom;
use super::save::wait_until;
use super::seed::SAVE_FILE;

pub(super) const NAME: &str = "paste_conversion";
const CLIPBOARD_OWNER_ENV: &str = "BEKOEDIT_PASTE_PROBE_CLIPBOARD_OWNER";

/// A heading, a paragraph with tag-based bold, a list and a table: every
/// structure §3.6 requires the end-to-end check to exercise.
const HTML_FLAVOUR: &str = "<h1>Report</h1><p>Some <b>bold</b> text.</p>\
    <ul><li>one</li><li>two</li></ul>\
    <table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>";
const PLAIN_FLAVOUR: &str = "Report\nSome bold text.\none\ntwo\na\tb\n1\t2";
/// The plain flavour's first line: enough to detect the plain paste landed,
/// without depending on how CodeMirror wraps or joins the rest.
const PLAIN_FIRST_LINE: &str = "Report";

const OWNER_READY_DEADLINE: Duration = Duration::from_secs(10);
const SETTLE: Duration = Duration::from_millis(1500);
const STEP_DEADLINE: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(50);

/// Installs a document-capture `paste` listener that only records what the
/// browser handed the page -- it never calls `preventDefault` or stops
/// propagation, so the product's own listener on the editor still runs
/// exactly as it would without this scenario watching (the same coexistence
/// `paste_probe.js` already relies on). `window.__bkPasteCapture.html` is
/// `null` until a `paste` with an HTML flavour is observed.
const CAPTURE_JS: &str = r#"
return (() => {
    window.__bkPasteCapture = { html: null, plainLength: null };
    document.addEventListener("paste", (event) => {
        const data = event.clipboardData;
        if (!data) return;
        const html = data.getData("text/html");
        window.__bkPasteCapture.html = html || null;
        window.__bkPasteCapture.plainLength = data.getData("text/plain").length;
    }, true);
    return true;
})();
"#;

async fn start_clipboard_owner(
    dir: &Path,
    script: Option<std::ffi::OsString>,
) -> Result<tokio::process::Child, String> {
    let Some(script) = script else {
        return Err(format!("{CLIPBOARD_OWNER_ENV} is not set"));
    };
    let ready_file = dir.join("ready");
    let stderr_file = dir.join("owner.stderr");
    let stderr = std::fs::File::create(&stderr_file)
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());
    let mut child = tokio::process::Command::new("python3")
        .arg(&script)
        .arg(HTML_FLAVOUR)
        .arg(PLAIN_FLAVOUR)
        .arg(&ready_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("{NAME}: could not start python3: {error}"))?;
    let started = tokio::time::Instant::now();
    loop {
        if ready_file.exists() {
            return Ok(child);
        }
        if let Ok(Some(status)) = child.try_wait() {
            let said = std::fs::read_to_string(&stderr_file).unwrap_or_default();
            return Err(format!(
                "{NAME}: the clipboard owner exited with {status}; stderr: {}",
                said.trim()
            ));
        }
        if started.elapsed() >= OWNER_READY_DEADLINE {
            return Err(format!(
                "{NAME}: the clipboard owner was not ready within {OWNER_READY_DEADLINE:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn editor_text() -> Result<String, String> {
    dom::run_script("return window.__bk?._view?.state.doc.toString() ?? null;").await
}

async fn captured_html() -> Result<Option<String>, String> {
    dom::run_script("return window.__bkPasteCapture?.html ?? null;").await
}

async fn wait_until_text_changes(from: &str) -> Result<String, String> {
    let started = tokio::time::Instant::now();
    loop {
        let now = editor_text().await?;
        if now != from {
            return Ok(now);
        }
        if started.elapsed() >= STEP_DEADLINE {
            return Err(format!(
                "{NAME}: the editor text never changed from its {}-character starting point",
                from.len()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Whether each of §3.6's four structures reads as its Markdown form in
/// `text`. A best-effort substring heuristic, for the report only -- never a
/// pass/fail signal (§3.0: it is an open question, not an assumption).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StructureSurvived {
    pub heading: bool,
    pub list: bool,
    pub bold: bool,
    pub table: bool,
}

pub(super) fn structure_report(markdown: &str) -> StructureSurvived {
    StructureSurvived {
        heading: markdown
            .lines()
            .any(|line| line.trim_start().starts_with('#')),
        list: markdown.lines().any(|line| {
            let t = line.trim_start();
            t.starts_with("- ") || t.starts_with("* ")
        }),
        bold: markdown.contains("**") || markdown.contains("__"),
        table: markdown.contains('|'),
    }
}

/// The saved file, with line endings normalized on both sides, must be
/// exactly `inserted` (as CodeMirror holds it, `\n`-only) followed by the
/// original content, also normalized. Tolerant of whatever line ending task
/// 027's reconciliation gives newly inserted content (its own concern, with
/// its own dedicated coverage in `save.rs`/`mode_switch.rs`): what this
/// guards is that paste's insertion corrupts or drops nothing on the way to
/// disk.
pub(super) fn check_saved_paste(
    original: &[u8],
    saved: &[u8],
    inserted: &str,
) -> Result<(), String> {
    let normalize = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let saved_normalized = normalize(saved);
    let expected = format!("{}{}", inserted.replace("\r\n", "\n"), normalize(original));
    if saved_normalized == expected {
        return Ok(());
    }
    Err(format!(
        "{NAME}: saved content (line endings normalized) does not match the converted paste \
         prefixed onto the original; saved {} chars, expected {} chars",
        saved_normalized.chars().count(),
        expected.chars().count()
    ))
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
    dom::run_script::<bool>(CAPTURE_JS).await?;

    let scratch =
        tempfile::tempdir().map_err(|error| format!("{NAME}: no scratch directory: {error}"))?;
    let _owner = start_clipboard_owner(scratch.path(), std::env::var_os(CLIPBOARD_OWNER_ENV))
        .await
        .map_err(|error| {
            format!(
                "{NAME}: {error} (never installed or run on this machine; \
            CI installs it under Xvfb)"
            )
        })?;

    let original_text = editor_text().await?;

    // §3.5: Ctrl+Shift+V pastes the plain flavour, unconverted, checked first
    // so its undo leaves the document exactly as the real paste will find it.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+Home"]).await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+shift+v"]).await?;
    wait_until(NAME, "the plain-paste chord to insert text", || {
        dom::editor_contains(PLAIN_FIRST_LINE)
    })
    .await?;
    let after_plain_paste = editor_text().await?;
    if after_plain_paste.contains("**") || after_plain_paste.contains('#') {
        return Err(format!(
            "{NAME}: the plain-paste chord left Markdown-looking content, so it was converted, \
             not pasted plain"
        ));
    }
    run_xdotool(&["key", "--clearmodifiers", "ctrl+z"]).await?;
    wait_until(NAME, "undo to remove the plain-pasted text", || async {
        Ok(!dom::editor_contains(PLAIN_FIRST_LINE).await?)
    })
    .await?;
    let reverted = editor_text().await?;
    if reverted != original_text {
        return Err(format!(
            "{NAME}: undo after the plain-paste check did not restore the original text"
        ));
    }

    // The real Ctrl+V: HTML present, converted, and inserted.
    run_xdotool(&["key", "--clearmodifiers", "ctrl+Home"]).await?;
    run_xdotool(&["key", "--clearmodifiers", "ctrl+v"]).await?;
    let _ = wait_until_text_changes(&original_text).await?;
    tokio::time::sleep(SETTLE).await;
    let after_paste = editor_text().await?;

    let received_html = captured_html()
        .await?
        .ok_or_else(|| format!("{NAME}: no text/html flavour was observed on the real paste"))?;
    // §3.0: this is the one required log line -- the full HTML the page
    // actually received, not what the owner served.
    println!(
        "  {NAME}: received text/html ({} chars, UTF-16 code units): {}",
        received_html.encode_utf16().count(),
        received_html
    );

    if !after_paste.ends_with(&original_text) {
        return Err(format!(
            "{NAME}: the converted paste's surrounding text was not left untouched \
             (expected the caret-0 insertion to leave the original as an exact suffix)"
        ));
    }
    let inserted = &after_paste[..after_paste.len() - original_text.len()];
    if inserted.is_empty() {
        return Err(format!(
            "{NAME}: the real paste inserted nothing (Empty outcome on non-empty HTML \
             is unexpected here)"
        ));
    }
    if inserted.contains("<h1") || inserted.contains("<p>") || inserted.contains("<b>") {
        return Err(format!(
            "{NAME}: raw HTML tags reached the document uninterpreted: {inserted:?}"
        ));
    }
    let structure = structure_report(inserted);
    println!(
        "  {NAME}: converted Markdown structure -- heading={} list={} bold={} table={}",
        structure.heading, structure.list, structure.bold, structure.table
    );
    println!("  {NAME}: converted Markdown: {inserted:?}");

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
                "{NAME}: the file on disk never changed after Ctrl+S"
            ));
        }
        tokio::time::sleep(POLL).await;
    }
    tokio::time::sleep(SETTLE).await;
    let saved = std::fs::read(file)
        .map_err(|error| format!("{NAME}: cannot read {}: {error}", file.display()))?;
    check_saved_paste(&expectation.original, &saved, inserted)?;

    Ok(vec![
        "Ctrl+Shift+V pasted the plain flavour, unconverted, and undo restored the original"
            .to_string(),
        format!(
            "a real Ctrl+V converted and saved; structure survived: heading={} list={} bold={} \
             table={} (not asserted -- see the logged HTML and RFC-046 §3.0)",
            structure.heading, structure.list, structure.bold, structure.table
        ),
        format!(
            "full received HTML logged ({} chars)",
            received_html.chars().count()
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_report_finds_each_marker_independently() {
        assert_eq!(
            structure_report("# Report\n\nSome **bold** text.\n\n- one\n- two\n\n| a | b |\n"),
            StructureSurvived {
                heading: true,
                list: true,
                bold: true,
                table: true
            }
        );
        assert_eq!(
            structure_report("Report\n\nSome bold text.\n\none\ntwo\n"),
            StructureSurvived::default()
        );
    }

    #[test]
    fn structure_report_accepts_asterisk_lists_and_underscore_bold() {
        let report = structure_report("* one\n* two\n\n__bold__\n");
        assert!(report.list && report.bold && !report.heading && !report.table);
    }

    #[test]
    fn check_saved_paste_accepts_an_exact_prefix_insertion() {
        let original = b"# Title\r\nbody\r\n";
        let inserted = "# Report\n\n";
        let mut saved = inserted.as_bytes().to_vec();
        saved.extend_from_slice(original);
        assert!(check_saved_paste(original, &saved, inserted).is_ok());
    }

    #[test]
    fn check_saved_paste_accepts_the_inserted_newlines_re_encoded_as_crlf() {
        let original = b"# Title\r\nbody\r\n";
        let inserted = "# Report\n\n";
        let mut saved = b"# Report\r\n\r\n".to_vec();
        saved.extend_from_slice(original);
        assert!(check_saved_paste(original, &saved, inserted).is_ok());
    }

    #[test]
    fn check_saved_paste_rejects_a_corrupted_or_dropped_insertion() {
        let original = b"# Title\r\nbody\r\n";
        let inserted = "# Report\n\n";
        let saved = original.to_vec(); // the paste never made it to disk
        let error = check_saved_paste(original, &saved, inserted).unwrap_err();
        assert!(error.contains("does not match"), "{error}");
    }

    #[test]
    fn check_saved_paste_rejects_a_change_to_the_untouched_original() {
        let original = b"# Title\r\nbody\r\n";
        let inserted = "# Report\n\n";
        let mut saved = inserted.as_bytes().to_vec();
        saved.extend_from_slice(b"# Title\r\nBODY\r\n"); // the original was touched
        let error = check_saved_paste(original, &saved, inserted).unwrap_err();
        assert!(error.contains("does not match"), "{error}");
    }
}
