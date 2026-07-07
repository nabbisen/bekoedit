//! The document session: canonical raw text plus derived projections
//! (RFC-006).
//!
//! Mutation paths:
//! - `apply_text_snapshot`: Text Mode replaces the canonical text wholesale
//!   after debounce (RFC-011 MVP strategy); allowed because Text Mode *is*
//!   the raw source editor.
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
    patch::apply_patch, render_preview_html,
};

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

    /// Form Mode update: semantic command -> validated minimal patch.
    pub fn apply_form_edit(&mut self, cmd: &FormEditCommand) -> Result<(), SessionError> {
        let patch =
            bekoedit_markdown::form::resolve_form_edit(&self.canonical_text, &self.index, cmd)?;
        apply_patch(&mut self.canonical_text, self.revision, &patch)?;
        self.after_mutation();
        Ok(())
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
