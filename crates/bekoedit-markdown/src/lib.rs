//! Markdown parser index, block identity, raw islands, source patch engine,
//! and form-edit resolution for bekoedit.
//!
//! Implements the source-preservation core specified by:
//! - RFC-013: Markdown Parser Index and Source Range Mapping
//! - RFC-014: Block Identity, Revision Scope, and Projection Validity
//! - RFC-015: SourcePatch Engine and Source-Preserving Mutation
//! - RFC-016/017/018: Form Mode projection, Raw Markdown Islands,
//!   and semantic edit commands (Rust-side resolution)
//!
//! Architectural invariants (RFC-000):
//! - The raw Markdown text is canonical; everything in this crate is a
//!   projection over it or a validated mutation of it.
//! - All byte ranges are Rust-owned UTF-8 byte offsets. Offsets supplied
//!   by the UI layer are advisory and never authoritative.
//! - Regions that cannot be edited safely become Raw Markdown Islands;
//!   they are never silently rewritten.

pub mod block;
pub mod fingerprint;
pub mod form;
pub mod index;
pub mod island;
pub mod patch;
pub mod preview;
pub mod range;
pub mod trivia;

pub use block::{BlockKind, BlockNode, EditablePolicy, HeadingNode, ListItemNode};
pub use fingerprint::{BlockFingerprint, BlockId};
pub use form::{
    FormBlock, FormBlockDisplay, FormBlockEdit, FormEditCommand, FormEditError, FormListItem,
    FormProjection, InlineFormat,
};
pub use index::{MarkdownDiagnostic, MarkdownIndex};
pub use island::{RawIsland, RawIslandEditPolicy, RawIslandType};
pub use patch::{PatchError, PatchOrigin, PatchResult, SourcePatch};
pub use preview::render_preview_html;
pub use range::{ByteRange, utf16_to_utf8_offset};
pub use trivia::{CodeFenceStyle, LineEnding, ListMarkerStyle, SourceTrivia};

#[cfg(test)]
mod tests;
