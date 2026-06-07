//! The SourcePatch engine (RFC-015): the only path through which canonical
//! Markdown text is mutated.
//!
//! Every patch is revision-checked and UTF-8-boundary-validated before
//! application, making invalid mutations impossible by construction
//! (reliability rule REL-103).

use serde::{Deserialize, Serialize};

use crate::range::{ByteRange, RangeError};

/// Where a patch originated (external design §23.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchOrigin {
    TextMode,
    FormMode,
    RawIsland,
    FileRecovery,
}

/// A Rust-approved mutation of a byte range in canonical text (RFC-015 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePatch {
    /// Document revision the patch was computed against.
    pub base_revision: u64,
    pub range: ByteRange,
    pub replacement: String,
    pub origin: PatchOrigin,
}

/// Result of a successfully applied patch (RFC-015 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchResult {
    /// Range covered by the replacement text in the new document.
    pub affected_range: ByteRange,
    /// Full reparse is always required in the MVP strategy.
    pub reparse_required: bool,
}

/// Structured patch rejection reasons (requirements §23.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum PatchError {
    #[error("document revision mismatch: patch base {base}, current {current}")]
    DocumentRevisionMismatch { base: u64, current: u64 },
    #[error("invalid patch range: {0}")]
    InvalidRange(#[from] RangeError),
}

/// Validates and applies `patch` to `text`, which must be at revision
/// `current_revision`. On success the caller increments the document
/// revision and triggers a full reparse.
pub fn apply_patch(
    text: &mut String,
    current_revision: u64,
    patch: &SourcePatch,
) -> Result<PatchResult, PatchError> {
    if patch.base_revision != current_revision {
        return Err(PatchError::DocumentRevisionMismatch {
            base: patch.base_revision,
            current: current_revision,
        });
    }
    patch.range.validate(text)?;
    text.replace_range(patch.range.start..patch.range.end, &patch.replacement);
    Ok(PatchResult {
        affected_range: ByteRange::new(
            patch.range.start,
            patch.range.start + patch.replacement.len(),
        ),
        reparse_required: true,
    })
}
