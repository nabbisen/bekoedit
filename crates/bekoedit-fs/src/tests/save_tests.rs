// Atomic save, recovery, recent workspaces, and settings tests.

use crate::paths::is_markdown_path;

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}


