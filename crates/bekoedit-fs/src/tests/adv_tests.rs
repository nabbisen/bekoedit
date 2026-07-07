// Backlinks, templates, history, and large-workspace tests.

use crate::paths::is_markdown_path;

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// --- RFC-034: backlinks ---

#[test]
fn backlinks_finds_standard_markdown_link() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("source.md"), "[see target](./target.md)\n").unwrap();
    std::fs::write(dir.path().join("target.md"), "# Target\n").unwrap();
    let links = crate::backlinks::find_backlinks(dir.path(), std::path::Path::new("target.md"));
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].source_path, std::path::Path::new("source.md"));
    assert_eq!(links[0].line_number, 1);
}

#[test]
fn backlinks_empty_when_no_references() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("a.md"), "no links here\n").unwrap();
    std::fs::write(dir.path().join("b.md"), "# B\n").unwrap();
    let links = crate::backlinks::find_backlinks(dir.path(), std::path::Path::new("b.md"));
    assert!(links.is_empty());
}

// --- RFC-037: templates ---

#[test]
fn template_listing_returns_empty_when_dir_absent() {
    let dir = temp_workspace();
    assert!(crate::templates::list_templates(dir.path()).is_empty());
}

#[test]
fn create_from_template_creates_prefilled_file() {
    let dir = temp_workspace();
    let created = crate::templates::create_from_template(
        dir.path(),
        std::path::Path::new(""),
        "note",
        "# My Note\n\n",
    )
    .unwrap();
    assert_eq!(created, std::path::Path::new("note.md"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
        "# My Note\n\n"
    );
}

// --- Local document history ---

#[test]
fn history_records_and_lists_entries() {
    let dir = temp_workspace();
    let store = crate::history::HistoryStore::at(dir.path().join("history"));
    let entry = crate::history::HistoryEntry {
        original_path: std::path::PathBuf::from("/ws/doc.md"),
        text: "# Version 1\n".to_string(),
        saved_at_secs: 1000,
        revision: 3,
    };
    store.record(&entry).unwrap();
    let entries = store.list(std::path::Path::new("/ws/doc.md"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].revision, 3);
}

#[test]
fn history_caps_at_max_and_prunes_oldest() {
    let dir = temp_workspace();
    let store = crate::history::HistoryStore::at(dir.path().join("history"));
    let path = std::path::PathBuf::from("/ws/doc.md");
    // Write 52 entries (2 over the cap of 50).
    for i in 0u64..52 {
        store
            .record(&crate::history::HistoryEntry {
                original_path: path.clone(),
                text: format!("rev {i}"),
                saved_at_secs: i,
                revision: i,
            })
            .unwrap();
    }
    let entries = store.list(&path);
    assert!(
        entries.len() <= 50,
        "history must be capped at 50, got {}",
        entries.len()
    );
    // Newest-first: first entry should be revision 51.
    assert_eq!(entries[0].revision, 51, "newest entry should be first");
}

#[test]
fn history_returns_empty_for_unknown_path() {
    let dir = temp_workspace();
    let store = crate::history::HistoryStore::at(dir.path().join("history"));
    let entries = store.list(std::path::Path::new("/ws/unknown.md"));
    assert!(entries.is_empty());
}

// --- Large workspace stress test ---

#[test]
fn file_tree_scans_500_markdown_files_without_panic() {
    let dir = temp_workspace();
    // Create 500 Markdown files in 20 subdirectories (25 per dir).
    for subdir in 0..20 {
        let d = dir.path().join(format!("dir{subdir:02}"));
        std::fs::create_dir_all(&d).unwrap();
        for file in 0..25 {
            std::fs::write(
                d.join(format!("note{file:03}.md")),
                format!("# Note {subdir}-{file}\n\nContent.\n"),
            )
            .unwrap();
        }
    }
    let start = std::time::Instant::now();
    let tree = crate::tree::FileTreeIndex::scan(dir.path(), &[]);
    let elapsed = start.elapsed();
    // nodes includes both files and directories: 500 files + 20 dirs = 520
    let file_count = tree
        .nodes
        .iter()
        .filter(|n| n.kind == crate::tree::FileNodeKind::MarkdownFile)
        .count();
    assert_eq!(
        file_count,
        500,
        "should find all 500 files; total nodes: {}",
        tree.nodes.len()
    );
    assert!(
        elapsed.as_millis() < 2000,
        "scan of 500 files took {} ms — should be under 2 s",
        elapsed.as_millis()
    );
}

#[test]
fn workspace_scan_ignores_node_modules_and_target() {
    let dir = temp_workspace();
    std::fs::create_dir_all(dir.path().join("node_modules").join("pkg")).unwrap();
    std::fs::write(
        dir.path()
            .join("node_modules")
            .join("pkg")
            .join("readme.md"),
        "",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("target").join("debug")).unwrap();
    std::fs::write(dir.path().join("target").join("debug").join("notes.md"), "").unwrap();
    std::fs::write(dir.path().join("real.md"), "# Real\n").unwrap();
    let tree = crate::tree::FileTreeIndex::scan(dir.path(), &[]);
    assert_eq!(
        tree.nodes.len(),
        1,
        "only the non-ignored file should appear"
    );
    assert_eq!(tree.nodes[0].display_name, "real.md");
}

// --- RFC-034: backlinks ---

// --- RFC-037: templates ---

// --- Local document history ---

// --- Large workspace stress test ---
