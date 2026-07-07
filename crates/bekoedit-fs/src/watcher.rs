//! Native filesystem watcher (RFC-005).
//!
//! Wraps `notify::RecommendedWatcher` (inotify on Linux, FSEvents on
//! macOS, ReadDirectoryChangesW on Windows) behind a simple poll-able
//! event queue. The watcher runs the OS machinery on its own internal
//! thread; callers drain events with `try_recv()` at any frequency they
//! choose, making it easy to merge with an existing tick loop without
//! adding a separate async channel.
//!
//! Design rules from RFC-005:
//! - Symlinked directories are not followed (SEC-003); they are excluded
//!   via the `RecursiveMode::Recursive` watcher, but the notify crate
//!   does not traverse into symlink targets on any platform by default.
//! - Events for ignored-directory entries are suppressed by the caller
//!   (we emit the raw path and let AppState decide relevance).
//! - `FsWatcher` is deliberately `Send` so it can live inside an async
//!   `use_future` closure without boxing workarounds.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Filesystem change events surfaced to the app layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A file's content was modified (covers both metadata and data changes
    /// so we don't miss platform-specific event variants).
    Modified(PathBuf),
    /// A new file or directory appeared.
    Created(PathBuf),
    /// A file or directory was removed.
    Deleted(PathBuf),
}

/// Holds a running filesystem watcher and a queue of pending events.
/// Drop to stop watching.
pub struct FsWatcher {
    /// Kept alive for the lifetime of the watcher (dropping stops it).
    _watcher: RecommendedWatcher,
    rx: Receiver<WatchEvent>,
}

impl FsWatcher {
    /// Starts watching `root` recursively. Returns `Err` only when the
    /// OS watcher cannot be initialised (e.g. inotify limit reached).
    pub fn start(root: &Path) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let kind_ev = match event.kind {
                        EventKind::Modify(_) => WatchEvent::Modified,
                        EventKind::Create(_) => WatchEvent::Created,
                        EventKind::Remove(_) => WatchEvent::Deleted,
                        _ => return,
                    };
                    for path in event.paths {
                        let _ = tx.send(kind_ev(path));
                    }
                }
            })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    /// Non-blocking drain: returns all pending events and clears the queue.
    pub fn drain(&self) -> Vec<WatchEvent> {
        self.rx.try_iter().collect()
    }
}

// Safety: the internal `mpsc::Receiver` is `Send` when `WatchEvent: Send`.
// `RecommendedWatcher` is `Send` on all supported platforms.
// The `FsWatcher` struct is therefore `Send`.
unsafe impl Send for FsWatcher {}
