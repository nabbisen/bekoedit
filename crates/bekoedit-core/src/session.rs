//! The document session: canonical raw text plus derived projections
//! (RFC-006).
//!
//! Mutation paths:
//! - `apply_editor_text`: Text Mode's snapshot, in editor form, reconciled
//!   with the canonical text so line endings survive (`editor_text.rs`,
//!   task 027).
//! - `apply_text_snapshot`: replaces the canonical text wholesale; for
//!   callers that already hold canonical-form text (section operations).
//! - `apply_form_edit`: Form Mode semantic commands are resolved into
//!   minimal source patches by the markdown crate; whole-document rewrite
//!   from Form Mode is impossible by construction (FM-006).
//!
//! Every accepted mutation increments the revision, marks the session
//! dirty, and triggers a full reparse (MVP simplicity invariant).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use bekoedit_fs::FileFingerprint;
use bekoedit_markdown::{
    FormEditCommand, FormEditError, FormProjection, LineEnding, MarkdownIndex, PatchError,
    PatchOrigin, SourcePatch, patch::apply_patch, render_preview_html,
};

use crate::form_history::{FormHistory, FormRecord};

/// Structured session errors (requirements §23.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum SessionError {
    #[error("text revision mismatch: client base {base}, current {current}")]
    TextRevisionMismatch { base: u64, current: u64 },
    #[error(transparent)]
    Form(#[from] FormEditError),
    #[error(transparent)]
    Patch(#[from] PatchError),
    #[error("file could not be read: {0}")]
    Read(String),
    #[error("file is not valid UTF-8; bekoedit edits UTF-8 Markdown only")]
    NotUtf8,
    /// Undo or Redo found the document changed since the edit it would
    /// reverse (RFC-049 §5.2). Both stacks were cleared; nothing was applied.
    #[error("the document changed since that edit, so it was not undone or redone")]
    FormHistoryStale,
}

/// One open Markdown document (RFC-006 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSession {
    pub document_id: u64,
    /// Absolute path of the backing file.
    pub path: PathBuf,
    pub canonical_text: String,
    pub line_ending: LineEnding,
    /// Incremented on every accepted mutation.
    pub revision: u64,
    /// True when memory differs from the last confirmed save.
    pub dirty: bool,
    /// Identity of the last-known on-disk content.
    /// `true` for in-memory files not yet saved to disk.
    pub is_untitled: bool,
    pub disk_fingerprint: Option<FileFingerprint>,
    pub index: MarkdownIndex,
    /// Undo and Redo of committed Form Mode edits (RFC-049). Not serialized;
    /// replacing the session discards it.
    #[serde(skip)]
    form_history: FormHistory,
}

impl DocumentSession {
    /// Creates a session over in-memory text (also used by tests and
    /// recovery restore).
    pub fn from_text(document_id: u64, path: PathBuf, text: String) -> Self {
        let line_ending = LineEnding::detect(&text);
        let index = MarkdownIndex::build(&text, 1);
        Self {
            document_id,
            path,
            canonical_text: text,
            line_ending,
            revision: 1,
            dirty: false,
            is_untitled: false,
            disk_fingerprint: None,
            index,
            form_history: FormHistory::default(),
        }
    }

    /// Loads a session from disk. Invalid UTF-8 is reported safely
    /// (RFC-006 acceptance) rather than lossily converted.
    /// Creates a blank in-memory session that has not been saved to disk.
    pub fn new_untitled(document_id: u64) -> Self {
        let path = std::env::temp_dir().join(format!("bekoedit-untitled-{document_id}.md"));
        let text = String::new();
        let index = MarkdownIndex::build(&text, 1);
        Self {
            document_id,
            path,
            line_ending: LineEnding::Lf,
            revision: 1,
            dirty: true,
            is_untitled: true,
            disk_fingerprint: None,
            index,
            canonical_text: text,
            form_history: FormHistory::default(),
        }
    }

    pub fn load(document_id: u64, path: &Path) -> Result<Self, SessionError> {
        let bytes = std::fs::read(path).map_err(|e| SessionError::Read(e.to_string()))?;
        let text = String::from_utf8(bytes).map_err(|_| SessionError::NotUtf8)?;
        let fingerprint = FileFingerprint::read(path).ok();
        let mut session = Self::from_text(document_id, path.to_path_buf(), text);
        session.disk_fingerprint = fingerprint;
        Ok(session)
    }

    /// Text Mode update: replaces canonical text after revision validation.
    pub fn apply_text_snapshot(
        &mut self,
        base_revision: u64,
        text: String,
    ) -> Result<(), SessionError> {
        if base_revision != self.revision {
            return Err(SessionError::TextRevisionMismatch {
                base: base_revision,
                current: self.revision,
            });
        }
        if text == self.canonical_text {
            return Ok(());
        }
        self.canonical_text = text;
        self.after_mutation();
        Ok(())
    }

    /// Whether `editor_text` (editor form) is exactly this document as the
    /// editor shows it -- Rule 0 of `editor_text.rs`.
    pub fn matches_editor_text(&self, editor_text: &str) -> bool {
        crate::editor_text::editor_form(editor_text)
            == crate::editor_text::editor_form(&self.canonical_text)
    }

    /// Text Mode update: reconciles the editor's text with the canonical text
    /// so the file's own line endings survive. Returns whether anything
    /// changed; under Rule 0 nothing does, and the revision and dirty flag
    /// are left alone.
    pub fn apply_editor_text(
        &mut self,
        base_revision: u64,
        editor_text: &str,
    ) -> Result<bool, SessionError> {
        if base_revision != self.revision {
            return Err(SessionError::TextRevisionMismatch {
                base: base_revision,
                current: self.revision,
            });
        }
        if self.matches_editor_text(editor_text) {
            return Ok(false);
        }
        self.canonical_text = crate::editor_text::reconcile(&self.canonical_text, editor_text);
        self.after_mutation();
        Ok(true)
    }

    /// Form Mode update: semantic command -> validated minimal patch.
    pub fn apply_form_edit(&mut self, cmd: &FormEditCommand) -> Result<(), SessionError> {
        let patch =
            bekoedit_markdown::form::resolve_form_edit(&self.canonical_text, &self.index, cmd)?;
        let before = self.revision;
        let replaced = self
            .canonical_text
            .get(patch.range.start..patch.range.end)
            .map(str::to_owned);
        let result = apply_patch(&mut self.canonical_text, self.revision, &patch)?;
        self.after_mutation();
        match replaced {
            Some(replaced) if replaced == patch.replacement => {
                self.form_history.restamp_tops(before, self.revision);
            }
            Some(replaced) => self.form_history.record(
                before,
                FormRecord {
                    revision_after: self.revision,
                    range_after: result.affected_range,
                    replaced,
                },
            ),
            // apply_patch validated the range, so this cannot happen.
            None => self.form_history.clear(),
        }
        Ok(())
    }

    /// Undoes the newest committed Form Mode edit. `Ok(false)` when there is
    /// none; an error, with both stacks cleared and the text untouched, when
    /// the document is no longer exactly as that edit left it.
    pub fn undo_form(&mut self) -> Result<bool, SessionError> {
        self.step_form_history(true)
    }

    /// Redoes the newest undone edit; the mirror of [`Self::undo_form`].
    pub fn redo_form(&mut self) -> Result<bool, SessionError> {
        self.step_form_history(false)
    }

    pub fn can_undo_form(&self) -> bool {
        self.form_history.depths().0 > 0
    }

    pub fn can_redo_form(&self) -> bool {
        self.form_history.depths().1 > 0
    }

    /// Empties both stacks (a mode switch away from Form Mode).
    pub fn clear_form_history(&mut self) {
        self.form_history.clear();
    }

    fn step_form_history(&mut self, undo: bool) -> Result<bool, SessionError> {
        let Some(top) = self.form_history.top_revision(undo) else {
            return Ok(false);
        };
        if top != self.revision {
            self.form_history.clear();
            return Err(SessionError::FormHistoryStale);
        }
        let record = self.form_history.pop(undo).expect("the top was just read");
        let range = record.range_after;
        let Some(current) = self
            .canonical_text
            .get(range.start..range.end)
            .map(str::to_owned)
        else {
            self.form_history.clear();
            return Err(SessionError::FormHistoryStale);
        };
        let patch = SourcePatch {
            base_revision: self.revision,
            range,
            replacement: record.replaced,
            origin: PatchOrigin::FormMode,
        };
        let result = match apply_patch(&mut self.canonical_text, self.revision, &patch) {
            Ok(result) => result,
            Err(error) => {
                self.form_history.clear();
                return Err(error.into());
            }
        };
        self.after_mutation();
        self.form_history.applied(
            undo,
            FormRecord {
                revision_after: self.revision,
                range_after: result.affected_range,
                replaced: current,
            },
            self.revision,
        );
        Ok(true)
    }

    /// Restore update: loads a saved/recovery snapshot as a new dirty edit.
    pub fn apply_restored_snapshot(&mut self, text: String) {
        self.canonical_text = text;
        self.after_mutation();
    }

    fn after_mutation(&mut self) {
        self.revision += 1;
        self.dirty = true;
        self.line_ending = LineEnding::detect(&self.canonical_text);
        self.index = MarkdownIndex::build(&self.canonical_text, self.revision);
    }

    /// Marks the session clean after a confirmed save.
    pub fn mark_saved(&mut self, fingerprint: FileFingerprint) {
        self.dirty = false;
        self.disk_fingerprint = Some(fingerprint);
    }

    /// Form Mode projection at the current revision.
    pub fn form_projection(&self) -> FormProjection {
        FormProjection::build(&self.canonical_text, &self.index)
    }

    /// Sanitized preview HTML at the current revision.
    pub fn preview_html(&self) -> String {
        render_preview_html(&self.canonical_text)
    }
    /// Returns `(word_count, char_count)` for the current canonical text.
    pub fn word_char_count(&self) -> (usize, usize) {
        let words = self.canonical_text.split_whitespace().count();
        let chars = self.canonical_text.chars().count();
        (words, chars)
    }
}
