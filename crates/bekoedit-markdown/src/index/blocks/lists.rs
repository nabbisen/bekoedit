//! List block classification (RFC-013): bullet, ordered, and task lists.
//!
//! Split out of `blocks.rs` to keep classification units within the
//! project's file-size guideline. Simple single-paragraph lists become
//! form-editable blocks with per-item content ranges; nested or
//! multi-paragraph lists downgrade to ComplexList raw islands.

use pulldown_cmark::{Event, Tag};

use crate::block::{BlockKind, ListItemNode};
use crate::island::RawIslandType;
use crate::range::ByteRange;
use crate::trivia::ListMarkerStyle;

use super::{Ev, PendingBlock, matching_end, trim_trailing_newlines};

pub(super) fn classify_list(
    text: &str,
    ordered_start: Option<u64>,
    subtree: &[Ev],
    start: usize,
    end: usize,
    base: usize,
) -> PendingBlock {
    let mut items: Vec<ListItemNode> = Vec::new();
    let mut has_task = false;
    let mut complex = false;

    let mut depth = 0i32;
    let mut j = 0usize;
    while j < subtree.len() {
        if depth == 1
            && let Event::Start(Tag::Item) = &subtree[j].0
        {
            let item_end_rel = matching_end(subtree, j);
            let item_subtree = &subtree[j..=item_end_rel];
            let istart = base + subtree[j].1.start;
            let iend = trim_trailing_newlines(text, base + subtree[j].1.end);
            let (item, item_complex) =
                classify_item(text, items.len() as u32, item_subtree, istart, iend);
            has_task |= item.task_checked.is_some();
            complex |= item_complex;
            items.push(item);
            // Skip the whole item subtree; net depth change is zero.
            j = item_end_rel + 1;
            continue;
        }
        match &subtree[j].0 {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
        j += 1;
    }

    let kind = if has_task {
        BlockKind::TaskList
    } else if ordered_start.is_some() {
        BlockKind::OrderedList
    } else {
        BlockKind::BulletList
    };
    let mut b = PendingBlock::new(kind, start, end);
    b.list_marker_style = items
        .first()
        .and_then(|it| ListMarkerStyle::detect(&text[it.source_range.start..it.source_range.end]));
    if complex {
        return b.island(
            RawIslandType::ComplexList,
            "nested or multi-block list items",
        );
    }
    b.items = items;
    b
}

/// Classifies a single list item; returns it plus a complexity flag.
fn classify_item(
    text: &str,
    ordinal: u32,
    subtree: &[Ev],
    start: usize,
    end: usize,
) -> (ListItemNode, bool) {
    let mut task_checked = None;
    let mut paragraphs = 0usize;
    let mut nested_blocks = 0usize;
    for (ev, _) in subtree.iter().skip(1) {
        match ev {
            Event::TaskListMarker(checked) => task_checked = Some(*checked),
            Event::Start(Tag::Paragraph) => paragraphs += 1,
            Event::Start(Tag::List(_))
            | Event::Start(Tag::BlockQuote(_))
            | Event::Start(Tag::CodeBlock(_))
            | Event::Start(Tag::Table(_))
            | Event::Start(Tag::HtmlBlock) => nested_blocks += 1,
            _ => {}
        }
    }
    let complex = nested_blocks > 0 || paragraphs > 1;
    let content_start = item_content_start(text, start, end, task_checked.is_some());
    let item = ListItemNode {
        ordinal,
        source_range: ByteRange::new(start, end),
        content_range: ByteRange::new(content_start.min(end), end),
        task_checked,
    };
    (item, complex)
}

/// Byte offset where the editable text of a list item begins
/// (after indentation, marker, one space, and optional task checkbox).
fn item_content_start(text: &str, start: usize, end: usize, has_task: bool) -> usize {
    let line = &text[start..end];
    let mut pos = line.len() - line.trim_start().len(); // indentation
    let rest = &line[pos..];
    if let Some(c) = rest.chars().next() {
        if c == '-' || c == '*' || c == '+' {
            pos += 1;
        } else if c.is_ascii_digit() {
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            pos += digits + 1; // digits plus `.` or `)`
        }
    }
    // One space after the marker.
    if line[pos..].starts_with(' ') {
        pos += 1;
    }
    if has_task {
        // `[x]` or `[ ]` plus one space.
        if line[pos..].starts_with('[') && line.len() >= pos + 3 {
            pos += 3;
            if line[pos..].starts_with(' ') {
                pos += 1;
            }
        }
    }
    start + pos
}
