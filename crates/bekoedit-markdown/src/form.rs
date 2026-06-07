//! Form Mode projection and semantic edit commands
//! (RFC-016 surface, RFC-017 islands, RFC-018 command set).
//!
//! The UI sends `FormEditCommand` values targeting revision-scoped block
//! identity (RFC-014). This module resolves them into minimal,
//! style-preserving `SourcePatch` values; it never rewrites unrelated
//! regions and never trusts client-supplied byte ranges.

mod resolve;

use serde::{Deserialize, Serialize};

use crate::block::{BlockKind, BlockNode, EditablePolicy};
use crate::fingerprint::{BlockFingerprint, BlockId};
use crate::index::MarkdownIndex;
use crate::island::RawIslandType;

pub use resolve::resolve_form_edit;

/// One visual block in the Form Mode projection (RFC-016 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormBlock {
    pub block_id: BlockId,
    pub kind: BlockKind,
    pub editable_policy: EditablePolicy,
    pub display: FormBlockDisplay,
}

/// Render-ready content for each supported block type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormBlockDisplay {
    Heading {
        level: u8,
        text: String,
        /// `false` for setext headings, whose level cannot be changed safely.
        level_editable: bool,
    },
    Paragraph {
        text: String,
    },
    List {
        ordered: bool,
        items: Vec<FormListItem>,
    },
    Blockquote {
        text: String,
    },
    Code {
        language: Option<String>,
        code: String,
    },
    HorizontalRule,
    RawIsland {
        island_type: RawIslandType,
        /// Translated by the GUI via i18n.
        label_key: String,
        text: String,
        editable: bool,
    },
}

/// One item inside a list `FormBlock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormListItem {
    pub ordinal: u32,
    pub text: String,
    pub task_checked: Option<bool>,
}

/// The Form Mode projection (external design §23.10). Disposable;
/// rebuilt from the `MarkdownIndex` after every accepted mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormProjection {
    pub document_revision: u64,
    pub blocks: Vec<FormBlock>,
}

impl FormProjection {
    /// Builds the projection for `index` over canonical `text`.
    pub fn build(text: &str, index: &MarkdownIndex) -> Self {
        let blocks = index
            .blocks
            .iter()
            .map(|b| FormBlock {
                block_id: b.block_id,
                kind: b.kind,
                editable_policy: b.editable_policy,
                display: display_for(text, index, b),
            })
            .collect();
        Self {
            document_revision: index.document_revision,
            blocks,
        }
    }
}

fn display_for(text: &str, index: &MarkdownIndex, block: &BlockNode) -> FormBlockDisplay {
    if block.editable_policy == EditablePolicy::RawIslandOnly {
        let island = index
            .raw_islands
            .iter()
            .find(|i| i.block_id == block.block_id);
        let island_type = island
            .map(|i| i.island_type)
            .unwrap_or(RawIslandType::UnknownExtension);
        return FormBlockDisplay::RawIsland {
            island_type,
            label_key: island_type.label_key().to_string(),
            text: slice(text, block.source_range.start, block.source_range.end),
            editable: true,
        };
    }
    let content = |r: Option<crate::range::ByteRange>| {
        r.map(|r| slice(text, r.start, r.end)).unwrap_or_default()
    };
    match block.kind {
        BlockKind::Heading => {
            let first = slice(text, block.source_range.start, block.source_range.end);
            let level_editable = first.trim_start().starts_with('#');
            FormBlockDisplay::Heading {
                level: block.heading_level.unwrap_or(1),
                text: content(block.content_range),
                level_editable,
            }
        }
        BlockKind::Paragraph => FormBlockDisplay::Paragraph {
            text: content(block.content_range),
        },
        BlockKind::Blockquote => FormBlockDisplay::Blockquote {
            text: content(block.content_range),
        },
        BlockKind::FencedCode => FormBlockDisplay::Code {
            language: block.code_language.clone(),
            code: content(block.content_range),
        },
        BlockKind::HorizontalRule => FormBlockDisplay::HorizontalRule,
        BlockKind::BulletList | BlockKind::OrderedList | BlockKind::TaskList => {
            FormBlockDisplay::List {
                ordered: block.kind == BlockKind::OrderedList,
                items: block
                    .items
                    .iter()
                    .map(|it| FormListItem {
                        ordinal: it.ordinal,
                        text: slice(text, it.content_range.start, it.content_range.end),
                        task_checked: it.task_checked,
                    })
                    .collect(),
            }
        }
        _ => FormBlockDisplay::RawIsland {
            island_type: RawIslandType::UnknownExtension,
            label_key: RawIslandType::UnknownExtension.label_key().to_string(),
            text: slice(text, block.source_range.start, block.source_range.end),
            editable: true,
        },
    }
}

fn slice(text: &str, start: usize, end: usize) -> String {
    text.get(start..end).unwrap_or_default().to_string()
}

/// Semantic edits a Form Mode block may request (RFC-018 §7, as amended
/// by the 2026-06-07 review to include `ReplaceListItemText` and
/// `DeleteBlock` per external design §23.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormBlockEdit {
    ReplacePlainText {
        text: String,
    },
    SetHeadingLevel {
        level: u8,
    },
    ToggleTaskChecked {
        item_ordinal: u32,
        checked: bool,
    },
    ReplaceListItemText {
        item_ordinal: u32,
        text: String,
    },
    ReplaceCodeBlock {
        language: Option<String>,
        code: String,
    },
    ReplaceRawIsland {
        text: String,
    },
    DeleteBlock,
}

/// A semantic edit command from the UI (RFC-018 §7). Carries no
/// authoritative byte ranges by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormEditCommand {
    pub base_revision: u64,
    pub block_id: BlockId,
    /// Optional client-side fingerprint for extra validation/diagnostics.
    pub client_block_fingerprint: Option<BlockFingerprint>,
    pub edit: FormBlockEdit,
}

/// Structured rejection reasons for Form Mode commands
/// (requirements §23.3, RFC-014 stale-command handling).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum FormEditError {
    #[error("document revision mismatch: command base {base}, current {current}")]
    DocumentRevisionMismatch { base: u64, current: u64 },
    #[error("block not found for the given id")]
    BlockNotFound,
    #[error("block fingerprint mismatch; projection is stale")]
    BlockFingerprintMismatch,
    #[error("list item {ordinal} not found")]
    ItemNotFound { ordinal: u32 },
    #[error("edit operation is not supported for this block: {reason}")]
    UnsupportedEditOperation { reason: String },
    #[error("invalid edit payload: {reason}")]
    InvalidEditPayload { reason: String },
}
