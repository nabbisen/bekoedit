//! The `MarkdownIndex`: a disposable, rebuildable projection of canonical
//! Markdown text into top-level blocks with Rust-owned source ranges
//! (RFC-013), revision-scoped identity (RFC-014), and Raw Markdown Islands
//! (RFC-017).
//!
//! MVP strategy: full reparse after every accepted mutation
//! (architectural invariant 7).

mod blocks;

use pulldown_cmark::{Event, Options, Parser};
use serde::{Deserialize, Serialize};

use crate::block::{BlockKind, BlockNode, EditablePolicy, HeadingNode};
use crate::fingerprint::{BlockFingerprint, BlockId};
use crate::island::{RawIsland, RawIslandEditPolicy, RawIslandType};
use crate::range::ByteRange;
use crate::trivia::SourceTrivia;

/// Non-fatal parse observation surfaced to the diagnostics panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownDiagnostic {
    pub message: String,
    pub range: Option<ByteRange>,
}

/// Derived index over canonical Markdown text (RFC-013 §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownIndex {
    pub document_revision: u64,
    pub blocks: Vec<BlockNode>,
    pub headings: Vec<HeadingNode>,
    pub raw_islands: Vec<RawIsland>,
    pub diagnostics: Vec<MarkdownDiagnostic>,
}

impl MarkdownIndex {
    /// Parses `text` into a fresh index tagged with `document_revision`.
    /// Parsing is tolerant (requirements §25.2): unrecognized or risky
    /// regions become raw islands, never load failures.
    pub fn build(text: &str, document_revision: u64) -> Self {
        let mut builder = Builder {
            text,
            revision: document_revision,
            blocks: Vec::new(),
            headings: Vec::new(),
            diagnostics: Vec::new(),
            island_types: Vec::new(),
        };

        let body_offset = match detect_front_matter(text) {
            Some(end) => {
                builder.push_front_matter(end);
                end
            }
            None => 0,
        };

        let body = &text[body_offset..];
        let events: Vec<(Event, std::ops::Range<usize>)> = Parser::new_ext(body, parse_options())
            .into_offset_iter()
            .collect();
        blocks::consume_top_level(&mut builder, &events, body_offset);

        let raw_islands = builder.collect_islands();
        MarkdownIndex {
            document_revision,
            blocks: builder.blocks,
            headings: builder.headings,
            raw_islands,
            diagnostics: builder.diagnostics,
        }
    }

    /// Resolves a revision-scoped `BlockId` against this index (RFC-014).
    /// Ordinal, kind, and fingerprint must all match; otherwise the command
    /// referencing the id must be rejected by the caller.
    pub fn resolve_block(&self, id: &BlockId) -> Option<&BlockNode> {
        let candidate = self.blocks.get(id.ordinal as usize)?;
        if candidate.kind == id.kind && candidate.block_id.fingerprint == id.fingerprint {
            Some(candidate)
        } else {
            None
        }
    }
}

/// Parser feature policy for MVP (requirements §25.3): CommonMark core plus
/// tables (to detect and preserve them as islands), task lists, strikethrough,
/// footnotes (preserved as islands), and math (preserved as islands).
fn parse_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
}

/// Detects YAML (`---`) or TOML (`+++`) front matter starting at byte 0.
/// Returns the byte offset just past the closing delimiter line (including
/// its newline), or `None` if no well-formed front matter is present.
pub(crate) fn detect_front_matter(text: &str) -> Option<usize> {
    let (open, close_alt) = if text.starts_with("---\n") || text.starts_with("---\r\n") {
        ("---", Some("..."))
    } else if text.starts_with("+++\n") || text.starts_with("+++\r\n") {
        ("+++", None)
    } else {
        return None;
    };
    let first_nl = text.find('\n')?;
    let mut offset = first_nl + 1;
    for line in text[offset..].split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        let line_end = offset + line.len();
        if trimmed == open || close_alt == Some(trimmed) {
            return Some(line_end);
        }
        offset = line_end;
    }
    None
}

/// Internal accumulation state while building an index.
pub(crate) struct Builder<'a> {
    pub text: &'a str,
    pub revision: u64,
    pub blocks: Vec<BlockNode>,
    pub headings: Vec<HeadingNode>,
    pub diagnostics: Vec<MarkdownDiagnostic>,
    island_types: Vec<(u32, RawIslandType, String)>,
}

impl Builder<'_> {
    fn push_front_matter(&mut self, end: usize) {
        let trimmed_end = blocks::trim_trailing_newlines(self.text, end);
        self.push_block(blocks::PendingBlock {
            kind: BlockKind::FrontMatter,
            start: 0,
            end: trimmed_end,
            content_range: None,
            editable_policy: EditablePolicy::RawIslandOnly,
            heading_level: None,
            items: Vec::new(),
            code_language: None,
            fence_style: None,
            list_marker_style: None,
            island: Some((RawIslandType::FrontMatter, "front matter".to_string())),
        });
    }

    pub(crate) fn push_block(&mut self, pending: blocks::PendingBlock) {
        let ordinal = self.blocks.len() as u32;
        let fingerprint = BlockFingerprint::compute(self.text, pending.start, pending.end);
        let mut trivia = SourceTrivia::compute(self.text, pending.start, pending.end);
        trivia.code_fence_style = pending.fence_style;
        trivia.list_marker_style = pending.list_marker_style;
        self.blocks.push(BlockNode {
            block_id: BlockId {
                revision_created: self.revision,
                ordinal,
                kind: pending.kind,
                fingerprint,
            },
            kind: pending.kind,
            source_range: ByteRange::new(pending.start, pending.end),
            content_range: pending.content_range,
            trivia,
            editable_policy: pending.editable_policy,
            heading_level: pending.heading_level,
            items: pending.items,
            code_language: pending.code_language,
        });
        if let Some((island_type, reason)) = pending.island {
            // Stored alongside the block; collected at the end.
            self.blocks.last_mut().expect("just pushed").editable_policy =
                EditablePolicy::RawIslandOnly;
            self.diagnostics.push(MarkdownDiagnostic {
                message: format!("raw island ({reason})"),
                range: Some(ByteRange::new(pending.start, pending.end)),
            });
            self.island_types.push((ordinal, island_type, reason));
        }
    }

    fn collect_islands(&self) -> Vec<RawIsland> {
        self.island_types
            .iter()
            .map(|(ordinal, island_type, reason)| {
                let block = &self.blocks[*ordinal as usize];
                RawIsland {
                    block_id: block.block_id,
                    island_type: *island_type,
                    source_range: block.source_range,
                    edit_policy: RawIslandEditPolicy::RawEditable,
                    reason: reason.clone(),
                }
            })
            .collect()
    }
}
