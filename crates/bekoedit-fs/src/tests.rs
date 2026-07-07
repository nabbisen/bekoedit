//! Tests for filesystem services, validating RFC-003/004/005/007
//! acceptance criteria and the SEC-001/002 path safety rules.

use std::path::Path;

use crate::atomic::{FileFingerprint, atomic_write};
use crate::ops::{
    DeleteStrategy, FileOpError, create_folder, create_markdown_file, delete_path, rename_path,
};
use crate::paths::{
    PathError, ensure_markdown_extension, resolve_in_workspace, sanitize_file_name,
};
use crate::recent::RecentWorkspaces;
use crate::recovery::{RecoverySnapshot, RecoveryStore};
use crate::tree::{FileNodeKind, FileTreeIndex};
use crate::workspace::{Workspace, WorkspaceError};

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

// --- paths (SEC-001/002) ---

#[test]
fn traversal_is_rejected() {
    let root = Path::new("/ws");
    assert_eq!(
        resolve_in_workspace(root, Path::new("../escape.md")).unwrap_err(),
        PathError::ParentTraversal
    );
    assert_eq!(
        resolve_in_workspace(root, Path::new("a/../../escape.md")).unwrap_err(),
        PathError::ParentTraversal
    );
    assert_eq!(
        resolve_in_workspace(root, Path::new("/abs.md")).unwrap_err(),
        PathError::AbsoluteNotAllowed
    );
}

#[test]
fn normal_relative_paths_resolve_inside_root() {
    let root = Path::new("/ws");
    let p = resolve_in_workspace(root, Path::new("notes/today.md")).unwrap();
    assert_eq!(p, Path::new("/ws/notes/today.md"));
}

#[test]
fn names_with_separators_are_rejected() {
    assert_eq!(
        sanitize_file_name("a/b").unwrap_err(),
        PathError::InvalidName
    );
    assert_eq!(
        sanitize_file_name("a\\b").unwrap_err(),
        PathError::InvalidName
    );
    assert_eq!(sanitize_file_name("  ").unwrap_err(), PathError::EmptyName);
    assert_eq!(
        sanitize_file_name("..").unwrap_err(),
        PathError::InvalidName
    );
    assert_eq!(sanitize_file_name("メモ.md").unwrap(), "メモ.md");
}

#[test]
fn markdown_extension_is_appended() {
    assert_eq!(ensure_markdown_extension("note"), "note.md");
    assert_eq!(ensure_markdown_extension("note.md"), "note.md");
    assert_eq!(ensure_markdown_extension("note.MARKDOWN"), "note.MARKDOWN");
}

// --- workspace (RFC-003) ---

#[test]
fn workspace_open_validates_path() {
    let dir = temp_workspace();
    let ws = Workspace::open(dir.path()).unwrap();
    assert!(!ws.display_name.is_empty());
    assert!(matches!(
        Workspace::open(Path::new("/definitely/missing/x")).unwrap_err(),
        WorkspaceError::Missing
    ));
}

// --- tree (RFC-004) ---

#[test]
fn tree_lists_markdown_and_ignores_noise() {
    let dir = temp_workspace();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules/junk")).unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join("README.md"), "# r").unwrap();
    std::fs::write(dir.path().join("docs/guide.markdown"), "# g").unwrap();
    std::fs::write(dir.path().join("binary.png"), [0u8, 1]).unwrap();
    std::fs::write(dir.path().join(".hidden.md"), "# h").unwrap();

    let tree = FileTreeIndex::scan(dir.path(), &[]);
    let names: Vec<&str> = tree.nodes.iter().map(|n| n.display_name.as_str()).collect();
    assert_eq!(names, vec!["docs", "guide.markdown", "README.md"]);
    assert_eq!(tree.nodes[0].kind, FileNodeKind::Directory);
    assert_eq!(tree.nodes[1].depth, 1, "nested file is depth 1");
    assert!(tree.warnings.is_empty());
}

#[test]
fn extra_ignore_patterns_apply() {
    let dir = temp_workspace();
    std::fs::create_dir_all(dir.path().join("drafts")).unwrap();
    std::fs::write(dir.path().join("drafts/x.md"), "x").unwrap();
    let tree = FileTreeIndex::scan(dir.path(), &["drafts".to_string()]);
    assert!(tree.nodes.is_empty());
}

// --- ops (RFC-005) ---

#[test]
fn create_rename_and_collision_rules() {
    let dir = temp_workspace();
    let rel = create_markdown_file(dir.path(), Path::new(""), "note").unwrap();
    assert_eq!(rel, Path::new("note.md"));
    assert!(dir.path().join("note.md").exists());
    assert!(matches!(
        create_markdown_file(dir.path(), Path::new(""), "note.md").unwrap_err(),
        FileOpError::AlreadyExists
    ));

    create_folder(dir.path(), Path::new(""), "sub").unwrap();
    let renamed = rename_path(dir.path(), Path::new("note.md"), "renamed.md").unwrap();
    assert_eq!(renamed, Path::new("renamed.md"));
    assert!(!dir.path().join("note.md").exists());
    assert!(dir.path().join("renamed.md").exists());
}

#[test]
fn permanent_delete_removes_file() {
    let dir = temp_workspace();
    create_markdown_file(dir.path(), Path::new(""), "gone").unwrap();
    delete_path(dir.path(), Path::new("gone.md"), DeleteStrategy::Permanent).unwrap();
    assert!(!dir.path().join("gone.md").exists());
}

#[test]
fn ops_reject_traversal() {
    let dir = temp_workspace();
    assert!(matches!(
        create_markdown_file(dir.path(), Path::new("../outside"), "x").unwrap_err(),
        FileOpError::Path(PathError::ParentTraversal)
    ));
}

// --- atomic save + fingerprints (RFC-007 / RFC-008) ---

#[test]
fn atomic_write_round_trips_and_fingerprints_detect_change() {
    let dir = temp_workspace();
    let file = dir.path().join("doc.md");
    let fp = atomic_write(&file, "# v1\n").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "# v1\n");
    assert!(!fp.disk_changed(&file).unwrap());

    // External modification is detected (RFC-005/008 acceptance).
    std::fs::write(&file, "# external\n").unwrap();
    assert!(fp.disk_changed(&file).unwrap());

    let fp2 = FileFingerprint::read(&file).unwrap();
    assert!(!fp2.disk_changed(&file).unwrap());
}

#[test]
fn atomic_write_leaves_no_temp_files() {
    let dir = temp_workspace();
    atomic_write(&dir.path().join("a.md"), "x").unwrap();
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("bekoedit-tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

// --- recovery (RFC-007) ---

#[test]
fn recovery_snapshots_persist_and_clear() {
    let dir = temp_workspace();
    let store = RecoveryStore::at(dir.path().join("recovery"));
    let snap = RecoverySnapshot {
        original_path: Path::new("/ws/doc.md").to_path_buf(),
        text: "unsaved 日本語".to_string(),
        revision: 7,
        created_at_secs: 1,
    };
    store.save(&snap).unwrap();
    let listed = store.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].text, "unsaved 日本語");
    store.remove(Path::new("/ws/doc.md")).unwrap();
    assert!(store.list().is_empty());
}

// --- recent workspaces (RFC-003) ---

#[test]
fn recent_workspaces_dedupe_and_survive_corrupt_files() {
    let dir = temp_workspace();
    let file = dir.path().join("recent.json");
    let mut recents = RecentWorkspaces::default();
    recents.record("/a".into(), "a".into(), 1);
    recents.record("/b".into(), "b".into(), 2);
    recents.record("/a".into(), "a".into(), 3);
    assert_eq!(recents.entries.len(), 2);
    assert_eq!(recents.entries[0].root_path, Path::new("/a"));
    recents.save(&file).unwrap();
    assert_eq!(RecentWorkspaces::load(&file), recents);

    std::fs::write(&file, "{ corrupt").unwrap();
    assert_eq!(RecentWorkspaces::load(&file), RecentWorkspaces::default());
}

// --- settings (RFC-022) ---

#[test]
fn user_settings_round_trip_and_defaults() {
    let dir = temp_workspace();
    let path = dir.path().join("settings.json");
    let s = crate::settings::UserSettings {
        autosave_debounce_ms: 800,
        extra_ignored_dirs: vec!["drafts".into()],
        ..Default::default()
    };
    s.save(&path).unwrap();
    let loaded = crate::settings::UserSettings::load(&path);
    assert_eq!(loaded.autosave_debounce_ms, 800);
    assert_eq!(loaded.extra_ignored_dirs, vec!["drafts"]);
    assert!(loaded.prefer_trash, "prefer_trash defaults to true");
}

#[test]
fn user_settings_corrupt_file_yields_defaults() {
    let dir = temp_workspace();
    let path = dir.path().join("bad-settings.json");
    std::fs::write(&path, "{ not json }").unwrap();
    let loaded = crate::settings::UserSettings::load(&path);
    assert_eq!(loaded, crate::settings::UserSettings::default());
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
