//! Backlink discovery (RFC-034).
//!
//! Scans all Markdown files under a workspace root for links that
//! reference a target document path, covering both standard Markdown
//! links `[text](./path.md)` and wiki-style links `[[page]]`.
//! Results are sorted by file path for deterministic output.

use std::path::{Path, PathBuf};

/// One reference to the target document found in a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkEntry {
    /// Workspace-relative path of the file containing the link.
    pub source_path: PathBuf,
    /// 1-based line number of the link.
    pub line_number: usize,
    /// The link text as written in the source document (trimmed, max 200 chars).
    pub context: String,
}

/// Finds all workspace files that contain a link to `target_rel`
/// (a workspace-relative path). Returns entries sorted by source path.
pub fn find_backlinks(root: &Path, target_rel: &Path) -> Vec<BacklinkEntry> {
    let target_stem = target_rel
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let target_str = target_rel.to_string_lossy().to_lowercase();
    // Also accept the bare stem (wiki-style) and the full relative path.
    let mut results: Vec<BacklinkEntry> = Vec::new();
    collect_backlinks(
        root,
        root,
        target_rel,
        &target_stem,
        &target_str,
        &mut results,
    );
    results.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    results
}

fn collect_backlinks(
    root: &Path,
    dir: &Path,
    target_rel: &Path,
    target_stem: &str,
    target_str: &str,
    out: &mut Vec<BacklinkEntry>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if super::tree::DEFAULT_IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(p);
        } else if super::paths::is_markdown_path(&p) {
            files.push(p);
        }
    }
    files.sort();
    dirs.sort();
    for file in files {
        if let Ok(rel) = file.strip_prefix(root) {
            // Skip the target file itself.
            if rel == target_rel {
                continue;
            }
            scan_file_for_backlinks(root, &file, rel, target_stem, target_str, out);
        }
    }
    for d in dirs {
        collect_backlinks(root, &d, target_rel, target_stem, target_str, out);
    }
}

fn scan_file_for_backlinks(
    root: &Path,
    path: &Path,
    rel: &Path,
    target_stem: &str,
    target_str: &str,
    out: &mut Vec<BacklinkEntry>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    for (i, line) in content.lines().enumerate() {
        let lower = line.to_lowercase();
        let is_link = lower.contains(target_str)
            || lower.contains(&format!("({target_str})"))
            || lower.contains(&format!("[[{target_stem}]]"))
            || lower.contains(&format!("[[{target_stem}|"))
            // Match bare filename links like [text](filename.md)
            || lower.contains(&format!(
                "({})",
                root.join(rel).parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|_| target_str.to_string())
                    .unwrap_or_default()
            ));
        if is_link && (lower.contains("](") || lower.contains("[[")) {
            out.push(BacklinkEntry {
                source_path: rel.to_path_buf(),
                line_number: i + 1,
                context: line.trim().chars().take(200).collect(),
            });
        }
    }
}
