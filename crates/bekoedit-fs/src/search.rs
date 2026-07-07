//! Workspace full-text search (RFC-033).
//!
//! Searches all Markdown files under a workspace root for a query string,
//! returning a ranked list of `SearchMatch` values ordered by relevance
//! (exact matches first, case-insensitive matches second). The search is
//! synchronous and intentionally simple — incremental indexing is post-MVP
//! (RFC-032). For workspaces with thousands of files, the caller should
//! run this on a background thread.

use std::path::{Path, PathBuf};

/// One matching line within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Workspace-relative path of the file.
    pub relative_path: PathBuf,
    /// 1-based line number of the match.
    pub line_number: usize,
    /// Full text of the matching line (trimmed to 200 chars).
    pub line_text: String,
    /// Whether the match was exact-case.
    pub exact_case: bool,
}

/// Searches all Markdown files under `root` for `query`.
/// Returns up to `limit` matches, exact-case results first.
pub fn search_workspace(root: &Path, query: &str, limit: usize) -> Vec<SearchMatch> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    let mut exact: Vec<SearchMatch> = Vec::new();
    let mut icase: Vec<SearchMatch> = Vec::new();

    collect_matches(
        root,
        root,
        query,
        &query_lower,
        &mut exact,
        &mut icase,
        limit,
    );

    exact.extend(icase);
    exact.truncate(limit);
    exact
}

fn collect_matches(
    root: &Path,
    dir: &Path,
    query: &str,
    query_lower: &str,
    exact: &mut Vec<SearchMatch>,
    icase: &mut Vec<SearchMatch>,
    limit: usize,
) {
    if exact.len() + icase.len() >= limit * 2 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if super::tree::DEFAULT_IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(path);
        } else if super::paths::is_markdown_path(&path) {
            files.push(path);
        }
    }
    files.sort();
    dirs.sort();
    for file in files {
        search_file(root, &file, query, query_lower, exact, icase);
    }
    for d in dirs {
        collect_matches(root, &d, query, query_lower, exact, icase, limit);
    }
}

fn search_file(
    root: &Path,
    path: &Path,
    query: &str,
    query_lower: &str,
    exact: &mut Vec<SearchMatch>,
    icase: &mut Vec<SearchMatch>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    for (i, line) in content.lines().enumerate() {
        if line.contains(query) {
            exact.push(SearchMatch {
                relative_path: rel.clone(),
                line_number: i + 1,
                line_text: truncate(line, 200),
                exact_case: true,
            });
        } else if line.to_lowercase().contains(query_lower) {
            icase.push(SearchMatch {
                relative_path: rel.clone(),
                line_number: i + 1,
                line_text: truncate(line, 200),
                exact_case: false,
            });
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() > max_chars {
        chars[..max_chars].iter().collect::<String>() + "…"
    } else {
        trimmed.to_string()
    }
}
