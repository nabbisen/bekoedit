//! Git status awareness (RFC-036).
//!
//! Optionally surfaces per-file Git status so the file explorer can show
//! modified/added/untracked indicators alongside Markdown files.
//! All errors are silent: if Git is not installed, the repository root is
//! not a Git repo, or the subprocess fails for any reason, the result is
//! an empty map and the rest of the application is unaffected.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The Git status of a tracked or untracked file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    /// The file has staged or unstaged modifications.
    Modified,
    /// The file is staged as new (added to the index).
    Added,
    /// The file has been deleted from the working tree.
    Deleted,
    /// The file is not tracked by Git.
    Untracked,
    /// The file has been renamed.
    Renamed,
}

/// Queries `git status --porcelain` in `repo_root` and returns a map of
/// workspace-relative paths to their status.
///
/// Returns an empty map if Git is unavailable, the directory is not a
/// repository, or any subprocess error occurs.
pub fn git_status_map(repo_root: &Path) -> HashMap<PathBuf, GitStatus> {
    let output = match std::process::Command::new("git")
        .args([
            "-C",
            &repo_root.to_string_lossy(),
            "status",
            "--porcelain",
            "-u",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return HashMap::new(),
    };

    let text = match String::from_utf8(output) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for line in text.lines() {
        if line.len() < 3 {
            continue;
        }
        let xy = &line[..2];
        let path_str = line[3..].trim();
        // For renames: "R  old -> new"; take the new path after " -> "
        let path_str = path_str.rsplit(" -> ").next().unwrap_or(path_str);
        let path = PathBuf::from(path_str);
        let status = parse_xy(xy);
        map.insert(path, status);
    }
    map
}

fn parse_xy(xy: &str) -> GitStatus {
    let bytes = xy.as_bytes();
    match (bytes.first().copied(), bytes.get(1).copied()) {
        (Some(b'?'), Some(b'?')) => GitStatus::Untracked,
        (Some(b'D'), _) | (_, Some(b'D')) => GitStatus::Deleted,
        (Some(b'R'), _) | (_, Some(b'R')) => GitStatus::Renamed,
        (Some(b'A'), _) => GitStatus::Added,
        _ => GitStatus::Modified,
    }
}
