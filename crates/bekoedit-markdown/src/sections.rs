//! Outline-based document section operations (RFC-029).
//!
//! A "section" is the range from one ATX heading to just before the next
//! heading of the same or higher level (lower number), or the end of the
//! document. Operations swap adjacent sibling sections; they never cross a
//! parent-level boundary, preserving document hierarchy.

use serde::{Deserialize, Serialize};

use crate::index::MarkdownIndex;
use crate::range::ByteRange;

/// Result of a section move operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionMoveResult {
    /// New canonical text after the sections are swapped.
    pub text: String,
    /// Byte offset of the heading in the new text (for cursor repositioning).
    pub new_heading_offset: usize,
}

/// Error conditions for section operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum SectionError {
    #[error("heading index {0} out of range")]
    HeadingIndexOutOfRange(usize),
    #[error("no sibling section exists in that direction")]
    NoSibling,
    #[error("section boundaries could not be resolved")]
    BoundaryError,
}

/// Computes the byte range of the section starting at `headings[idx]`.
/// The section ends just before the next heading of equal or higher level
/// (numerically ≤ the current heading level), or at end-of-document.
pub fn section_range(text: &str, index: &MarkdownIndex, heading_idx: usize) -> Option<ByteRange> {
    let headings = &index.headings;
    if heading_idx >= headings.len() {
        return None;
    }
    let start = headings[heading_idx].source_range.start;
    let level = headings[heading_idx].level;

    // Find the next heading at the same or higher level.
    let end = headings[heading_idx + 1..]
        .iter()
        .find(|h| h.level <= level)
        .map(|h| h.source_range.start)
        .unwrap_or(text.len());

    Some(ByteRange::new(start, end))
}

/// Moves the section at `heading_idx` one position earlier among its
/// siblings (swaps with the preceding sibling section).
pub fn move_section_up(
    text: &str,
    index: &MarkdownIndex,
    heading_idx: usize,
) -> Result<SectionMoveResult, SectionError> {
    if heading_idx >= index.headings.len() {
        return Err(SectionError::HeadingIndexOutOfRange(heading_idx));
    }
    let level = index.headings[heading_idx].level;

    // Find the preceding sibling (same level, no intervening higher-level heading).
    let prev_idx = (0..heading_idx)
        .rev()
        .find(|&i| index.headings[i].level == level)
        .ok_or(SectionError::NoSibling)?;

    // Verify no heading with a higher level (lower number) exists between them.
    let has_parent_between = (prev_idx + 1..heading_idx).any(|i| index.headings[i].level < level);
    if has_parent_between {
        return Err(SectionError::NoSibling);
    }

    let range_prev = section_range(text, index, prev_idx).ok_or(SectionError::BoundaryError)?;
    let range_curr = section_range(text, index, heading_idx).ok_or(SectionError::BoundaryError)?;

    swap_sections(text, range_prev, range_curr)
}

/// Moves the section at `heading_idx` one position later among its siblings.
pub fn move_section_down(
    text: &str,
    index: &MarkdownIndex,
    heading_idx: usize,
) -> Result<SectionMoveResult, SectionError> {
    if heading_idx >= index.headings.len() {
        return Err(SectionError::HeadingIndexOutOfRange(heading_idx));
    }
    let level = index.headings[heading_idx].level;

    // Find the next sibling (same level, no intervening higher-level heading).
    let next_idx = (heading_idx + 1..index.headings.len())
        .find(|&i| index.headings[i].level == level)
        .ok_or(SectionError::NoSibling)?;

    let has_parent_between = (heading_idx + 1..next_idx).any(|i| index.headings[i].level < level);
    if has_parent_between {
        return Err(SectionError::NoSibling);
    }

    let range_curr = section_range(text, index, heading_idx).ok_or(SectionError::BoundaryError)?;
    let range_next = section_range(text, index, next_idx).ok_or(SectionError::BoundaryError)?;

    let result = swap_sections(text, range_curr, range_next)?;
    // heading is now at range_next.start after the swap
    Ok(result)
}

fn swap_sections(
    text: &str,
    first: ByteRange,
    second: ByteRange,
) -> Result<SectionMoveResult, SectionError> {
    // Sections must be contiguous: first.end == second.start (possibly
    // with intervening blank lines that belong to neither).
    // We keep the text between sections (blank lines) attached to the
    // second section header so spacing is preserved.
    let first_text = &text[first.start..first.end];
    let second_text = &text[second.start..second.end];
    let gap = &text[first.end..second.start];

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..first.start]);
    result.push_str(second_text);
    result.push_str(gap);
    result.push_str(first_text);
    result.push_str(&text[second.end..]);

    let new_heading_offset = first.start;
    Ok(SectionMoveResult {
        text: result,
        new_heading_offset,
    })
}
