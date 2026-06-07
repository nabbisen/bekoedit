//! Save lifecycle state and the debounced autosave scheduler (RFC-007).
//!
//! The scheduler is pure: it takes the caller's notion of "now" in
//! milliseconds, which makes the debounce behavior deterministic and
//! testable (RFC-007 internal notes).

use serde::{Deserialize, Serialize};

/// User-visible save lifecycle (external design §24.1, simplified for the
/// single-document MVP store).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveState {
    Clean,
    Dirty,
    AutoSaveScheduled { due_at_ms: u64 },
    Saving,
    Saved { at_ms: u64 },
    SaveFailed { message: String, retryable: bool },
    ConflictResolutionRequired,
}

impl SaveState {
    /// Status-bar label key, translated by the GUI (i18n).
    pub fn label_key(&self) -> &'static str {
        match self {
            SaveState::Clean => "save.clean",
            SaveState::Dirty => "save.dirty",
            SaveState::AutoSaveScheduled { .. } => "save.scheduled",
            SaveState::Saving => "save.saving",
            SaveState::Saved { .. } => "save.saved",
            SaveState::SaveFailed { .. } => "save.failed",
            SaveState::ConflictResolutionRequired => "save.conflict",
        }
    }
}

/// Debounced autosave policy: do not write on every keystroke; reschedule
/// while edits keep arriving; pause entirely during conflicts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutosaveScheduler {
    pub debounce_ms: u64,
    pub enabled: bool,
    due_at_ms: Option<u64>,
    paused: bool,
}

impl AutosaveScheduler {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            debounce_ms,
            enabled: true,
            due_at_ms: None,
            paused: false,
        }
    }

    /// Called after every accepted edit; (re)schedules the next autosave.
    pub fn note_edit(&mut self, now_ms: u64) {
        if self.enabled && !self.paused {
            self.due_at_ms = Some(now_ms + self.debounce_ms);
        }
    }

    /// True when a scheduled autosave has become due.
    pub fn is_due(&self, now_ms: u64) -> bool {
        !self.paused && self.due_at_ms.is_some_and(|due| now_ms >= due)
    }

    pub fn due_at(&self) -> Option<u64> {
        self.due_at_ms
    }

    /// Clears the pending autosave (after a save or document close).
    pub fn clear(&mut self) {
        self.due_at_ms = None;
    }

    /// Pauses autosave; used while a conflict awaits resolution
    /// (external design §19.4: autosave must pause for conflicted documents).
    pub fn pause(&mut self) {
        self.paused = true;
        self.due_at_ms = None;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }
}
