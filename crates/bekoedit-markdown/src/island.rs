//! Raw Markdown Islands (RFC-017).
//!
//! Islands are the safe-fallback representation for any region that
//! Form Mode cannot edit structurally. Their source bytes are preserved
//! exactly unless the user edits the island text directly.

use serde::{Deserialize, Serialize};

use crate::fingerprint::BlockId;
use crate::range::ByteRange;

/// Categories of raw islands (external design §23.9, RFC-017 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawIslandType {
    FrontMatter,
    HtmlBlock,
    ComplexTable,
    MathBlock,
    Directive,
    ComplexList,
    ComplexBlockquote,
    UnknownExtension,
    MalformedRegion,
}

impl RawIslandType {
    /// User-facing label key; the GUI translates it (i18n).
    pub fn label_key(&self) -> &'static str {
        match self {
            RawIslandType::FrontMatter => "island.front_matter",
            RawIslandType::HtmlBlock => "island.html_block",
            RawIslandType::ComplexTable => "island.complex_table",
            RawIslandType::MathBlock => "island.math_block",
            RawIslandType::Directive => "island.directive",
            RawIslandType::ComplexList => "island.complex_list",
            RawIslandType::ComplexBlockquote => "island.complex_blockquote",
            RawIslandType::UnknownExtension => "island.unknown_extension",
            RawIslandType::MalformedRegion => "island.malformed_region",
        }
    }
}

/// Whether the island can be edited as raw text inside Form Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawIslandEditPolicy {
    RawEditable,
    ReadOnlyWithTextModeEscape,
}

/// One preserved region (RFC-017 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawIsland {
    pub block_id: BlockId,
    pub island_type: RawIslandType,
    /// Rust-owned, revision-scoped range of the preserved source bytes.
    pub source_range: ByteRange,
    pub edit_policy: RawIslandEditPolicy,
    /// Human-readable reason (developer/diagnostics oriented; the GUI uses
    /// `island_type.label_key()` for translated labels).
    pub reason: String,
}
