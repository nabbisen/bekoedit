// Path validation, workspace, file tree, and file-op tests.

use std::path::Path;

use crate::ops::FileOpError;
use crate::ops::{DeleteStrategy, create_folder, create_markdown_file, delete_path, rename_path};
use crate::paths::{
    PathError, ensure_markdown_extension, is_markdown_path, resolve_in_workspace,
    sanitize_file_name,
};
use crate::tree::{FileNodeKind, FileTreeIndex};
use crate::workspace::{Workspace, WorkspaceError};

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
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
