//! Block-level model produced by the Markdown indexer (RFC-013).

use serde::{Deserialize, Serialize};

use crate::fingerprint::BlockId;
use crate::range::ByteRange;
use crate::trivia::SourceTrivia;

/// Top-level block categories recognized by the indexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockKind {
    Heading,
    Paragraph,
    BulletList,
    OrderedList,
    TaskList,
    Blockquote,
    FencedCode,
    HorizontalRule,
    HtmlBlock,
    FrontMatter,
    /// A simple NxM table where all cells are plain text (RFC-027).
    SimpleTable,
    /// A table with complex cells (merged, nested formatting, etc.).
    ComplexTable,
    /// Anything the indexer does not positively recognize as safe.
    Unknown,
}

/// Whether (and how) Form Mode may edit a block.
///
/// Per the safe-fallback invariant, the indexer prefers false negatives:
/// when uncertain, a block is downgraded to `RawIslandOnly`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditablePolicy {
    /// Structured semantic edits are allowed (e.g. paragraph text, heading level).
    FormEditable,
    /// Only raw-text island editing is allowed; the source is shown verbatim.
    RawIslandOnly,
    /// Visual block with delete action only (e.g. horizontal rule).
    DeleteOnly,
}

/// One list item inside a (task/bullet/ordered) list block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItemNode {
    /// Zero-based ordinal within the parent block, revision-scoped (RFC-018).
    pub ordinal: u32,
    /// Full source range of the item line(s), excluding the trailing newline.
    pub source_range: ByteRange,
    /// Range of the editable item text (after marker and optional checkbox).
    pub content_range: ByteRange,
    /// Checkbox state for task items; `None` for plain items.
    pub task_checked: Option<bool>,
}

/// Heading entry for the outline projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingNode {
    pub level: u8,
    pub text: String,
    pub source_range: ByteRange,
}

/// A parsed top-level block with Rust-owned source ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockNode {
    pub block_id: BlockId,
    pub kind: BlockKind,
    /// Full source range of the block, excluding surrounding blank lines.
    pub source_range: ByteRange,
    /// Range of the directly editable content, when the kind supports it:
    /// heading text after the marker, paragraph text, fenced code body, …
    pub content_range: Option<ByteRange>,
    pub trivia: SourceTrivia,
    pub editable_policy: EditablePolicy,
    /// Heading level for `Heading` blocks.
    pub heading_level: Option<u8>,
    /// Items for list blocks.
    pub items: Vec<ListItemNode>,
    /// Code fence info string (language) for `FencedCode` blocks.
    pub code_language: Option<String>,
}

impl BlockNode {
    pub fn is_form_editable(&self) -> bool {
        matches!(
            self.editable_policy,
            EditablePolicy::FormEditable | EditablePolicy::DeleteOnly
        )
    }
}
