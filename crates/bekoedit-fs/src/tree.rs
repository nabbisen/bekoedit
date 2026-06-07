//! Workspace file tree index (RFC-004).
//!
//! The tree is a projection: the UI receives `FileTreeNode` values, never
//! raw filesystem handles. Traversal is conservative: symlinked directories
//! are not followed (SEC-003), inaccessible directories become non-fatal
//! warnings, and high-noise directories are ignored by default.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::is_markdown_path;

/// Directories excluded by default (requirements §13.3, ER-003).
pub const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".next",
    ".svelte-kit",
    "dist",
    "build",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileNodeKind {
    Directory,
    MarkdownFile,
}

/// One row of the explorer tree projection (RFC-004 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTreeNode {
    /// Stable id: the workspace-relative path (RFC-004 internal notes).
    pub relative_path: PathBuf,
    pub display_name: String,
    pub kind: FileNodeKind,
    pub depth: u16,
}

/// The flattened, depth-annotated tree projection (RFC-004 §7).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FileTreeIndex {
    pub nodes: Vec<FileTreeNode>,
    /// Non-fatal traversal warnings (permission errors etc.).
    pub warnings: Vec<String>,
}

impl FileTreeIndex {
    /// Scans `root`, listing directories and Markdown files. Hidden entries
    /// (dot-prefixed) and `extra_ignored` directories are skipped. Entries
    /// are sorted directories-first, then case-insensitively by name.
    pub fn scan(root: &Path, extra_ignored: &[String]) -> Self {
        let mut index = FileTreeIndex::default();
        scan_dir(root, root, 0, extra_ignored, &mut index);
        index
    }
}

fn is_ignored(name: &str, extra: &[String]) -> bool {
    name.starts_with('.') || DEFAULT_IGNORED_DIRS.contains(&name) || extra.iter().any(|e| e == name)
}

fn scan_dir(root: &Path, dir: &Path, depth: u16, extra: &[String], out: &mut FileTreeIndex) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            out.warnings.push(format!("{}: {err}", dir.display()));
            return;
        }
    };
    let mut dirs: Vec<(String, PathBuf)> = Vec::new();
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Do not follow symlinked directories (conservative policy).
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() && !file_type.is_symlink() {
            if !is_ignored(&name, extra) {
                dirs.push((name, path));
            }
        } else if file_type.is_file() && !name.starts_with('.') && is_markdown_path(&path) {
            files.push((name, path));
        }
    }
    let sort_key = |s: &str| s.to_lowercase();
    dirs.sort_by_key(|(n, _)| sort_key(n));
    files.sort_by_key(|(n, _)| sort_key(n));

    for (name, path) in dirs {
        out.nodes
            .push(node(root, &path, &name, FileNodeKind::Directory, depth));
        scan_dir(root, &path, depth + 1, extra, out);
    }
    for (name, path) in files {
        out.nodes
            .push(node(root, &path, &name, FileNodeKind::MarkdownFile, depth));
    }
}

fn node(root: &Path, path: &Path, name: &str, kind: FileNodeKind, depth: u16) -> FileTreeNode {
    FileTreeNode {
        relative_path: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
        display_name: name.to_string(),
        kind,
        depth,
    }
}
