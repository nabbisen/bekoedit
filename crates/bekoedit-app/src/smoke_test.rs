//! Headless smoke test (RFC-025 CI acceptance).
//!
//! Invoked with `bekoedit --headless-smoke`. Exercises the core Rust code
//! paths — workspace open, document load, parse, form edit, save, conflict
//! detection — without starting the Dioxus Desktop event loop. Exit 0 on
//! success; non-zero on any failure.

use std::path::Path;

use bekoedit_core::{AppState, ConflictState};
use bekoedit_fs::RecoveryStore;
use bekoedit_markdown::{FormBlockEdit, FormEditCommand, MarkdownIndex};

/// Runs all smoke checks. Panics on failure (non-zero exit via panic handler).
pub fn run() {
    println!("bekoedit headless smoke test");

    // ── 1. Source preservation engine ───────────────────────────────────────
    let doc = "# Hello\n\nworld\n";
    let idx = MarkdownIndex::build(doc, 1);
    assert_eq!(idx.headings.len(), 1, "heading count");
    assert_eq!(idx.headings[0].text, "Hello", "heading text");
    let block = idx
        .blocks
        .iter()
        .find(|b| b.kind == bekoedit_markdown::BlockKind::Paragraph)
        .expect("paragraph block");
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: block.block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: "smoke".into(),
        },
    };
    let patch =
        bekoedit_markdown::form::resolve_form_edit(doc, &idx, &cmd).expect("resolve form edit");
    let mut edited = doc.to_string();
    bekoedit_markdown::patch::apply_patch(&mut edited, 1, &patch).expect("apply patch");
    assert_eq!(edited, "# Hello\n\nsmoke\n", "form edit patch");
    println!("  ✓ source preservation engine");

    // ── 2. Filesystem layer ──────────────────────────────────────────────────
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("test.md"), "# Smoke\n").unwrap();
    let tree = bekoedit_fs::FileTreeIndex::scan(root, &[]);
    assert_eq!(tree.nodes.len(), 1, "file tree node count");
    println!("  ✓ filesystem layer");

    // ── 3. Application state ────────────────────────────────────────────────
    let mut state = AppState::new(
        RecoveryStore::at(root.join(".recovery")),
        root.join(".recent.json"),
        1500,
    );
    state.open_workspace(root, 0).expect("open workspace");
    state
        .open_document(Path::new("test.md"))
        .expect("open document");
    let rev = state.session.as_ref().unwrap().revision;
    state
        .edit_text(rev, "# Smoke\n\nedited\n".into(), 1000)
        .expect("edit text");
    assert!(state.session.as_ref().unwrap().dirty, "dirty after edit");
    state.save_now(1000).expect("save now");
    assert!(!state.session.as_ref().unwrap().dirty, "clean after save");
    println!("  ✓ application state (open/edit/save)");

    // ── 4. Conflict detection ────────────────────────────────────────────────
    // Simulate external modification
    let doc_path = root.join("test.md");
    std::fs::write(&doc_path, "# External edit\n").unwrap();
    let rev2 = state.session.as_ref().unwrap().revision;
    state
        .edit_text(rev2, "# My edit\n".into(), 2000)
        .expect("edit");
    let conflict = state.check_external_change();
    assert!(
        conflict == ConflictState::DiskChangedDirtyMemory,
        "conflict not detected: {conflict:?}"
    );
    println!("  ✓ conflict detection");

    // ── 5. Section operations ────────────────────────────────────────────────
    let two_sections = "# A\n\nbody A\n\n# B\n\nbody B\n";
    let idx2 = MarkdownIndex::build(two_sections, 1);
    let result =
        bekoedit_markdown::move_section_down(two_sections, &idx2, 0).expect("move section down");
    assert!(result.text.contains("# B"), "section move");
    println!("  ✓ section operations");

    println!("bekoedit smoke test PASSED");
}
