//! Top-level block classification for the Markdown indexer.
//!
//! Safety stance (RFC-013 internal notes): prefer false negatives —
//! anything that cannot be mapped to a safe editable structure becomes
//! a Raw Markdown Island.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag};

use crate::block::{BlockKind, EditablePolicy, HeadingNode, ListItemNode};
use crate::island::RawIslandType;
use crate::range::ByteRange;
use crate::trivia::{CodeFenceStyle, ListMarkerStyle};

use super::Builder;

mod lists;

type Ev<'a> = (Event<'a>, std::ops::Range<usize>);

/// Block under construction, before identity/trivia are attached.
pub(crate) struct PendingBlock {
    pub kind: BlockKind,
    pub start: usize,
    pub end: usize,
    pub content_range: Option<ByteRange>,
    pub editable_policy: EditablePolicy,
    pub heading_level: Option<u8>,
    pub items: Vec<ListItemNode>,
    pub code_language: Option<String>,
    pub fence_style: Option<CodeFenceStyle>,
    pub list_marker_style: Option<ListMarkerStyle>,
    pub island: Option<(RawIslandType, String)>,
}

impl PendingBlock {
    fn new(kind: BlockKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            start,
            end,
            content_range: None,
            editable_policy: EditablePolicy::FormEditable,
            heading_level: None,
            items: Vec::new(),
            code_language: None,
            fence_style: None,
            list_marker_style: None,
            island: None,
        }
    }

    fn island(mut self, t: RawIslandType, reason: &str) -> Self {
        self.editable_policy = EditablePolicy::RawIslandOnly;
        self.island = Some((t, reason.to_string()));
        self
    }
}

/// Walks the offset-event stream and emits one `PendingBlock` per top-level
/// block. `base` is the byte offset of the parsed body within the canonical
/// text (non-zero when front matter was stripped).
pub(crate) fn consume_top_level(builder: &mut Builder, events: &[Ev], base: usize) {
    let mut i = 0;
    while i < events.len() {
        match &events[i].0 {
            Event::Start(tag) => {
                let end_idx = matching_end(events, i);
                let subtree = &events[i..=end_idx];
                let start = base + events[i].1.start;
                let raw_end = base + events[i].1.end;
                let end = trim_trailing_newlines(builder.text, raw_end);
                let pending = classify(builder, tag.clone(), subtree, start, end, base);
                builder.push_block(pending);
                i = end_idx + 1;
            }
            Event::Rule => {
                let start = base + events[i].1.start;
                let end = trim_trailing_newlines(builder.text, base + events[i].1.end);
                let mut b = PendingBlock::new(BlockKind::HorizontalRule, start, end);
                b.editable_policy = EditablePolicy::DeleteOnly;
                builder.push_block(b);
                i += 1;
            }
            _ => i += 1,
        }
    }
}

/// Index of the `End` event matching the `Start` at `start_idx`.
fn matching_end(events: &[Ev], start_idx: usize) -> usize {
    let mut depth = 0i32;
    for (j, (ev, _)) in events.iter().enumerate().skip(start_idx) {
        match ev {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
            _ => {}
        }
    }
    events.len() - 1
}

fn classify(
    builder: &mut Builder,
    tag: Tag,
    subtree: &[Ev],
    start: usize,
    end: usize,
    base: usize,
) -> PendingBlock {
    match tag {
        Tag::Heading { level, .. } => classify_heading(builder, level, subtree, start, end),
        Tag::Paragraph => classify_paragraph(subtree, start, end),
        Tag::List(ordered_start) => {
            lists::classify_list(builder.text, ordered_start, subtree, start, end, base)
        }
        Tag::BlockQuote(_) => classify_blockquote(builder.text, subtree, start, end),
        Tag::CodeBlock(kind) => classify_code(builder, kind, start, end),
        Tag::HtmlBlock => PendingBlock::new(BlockKind::HtmlBlock, start, end)
            .island(RawIslandType::HtmlBlock, "HTML block"),
        Tag::Table(_) => classify_table(subtree, start, end),
        Tag::FootnoteDefinition(_) => PendingBlock::new(BlockKind::Unknown, start, end)
            .island(RawIslandType::Footnote, "footnote definition"),
        _ => PendingBlock::new(BlockKind::Unknown, start, end)
            .island(RawIslandType::UnknownExtension, "unsupported block"),
    }
}

fn classify_heading(
    builder: &mut Builder,
    level: HeadingLevel,
    subtree: &[Ev],
    start: usize,
    end: usize,
) -> PendingBlock {
    let level = heading_level_u8(level);
    let mut b = PendingBlock::new(BlockKind::Heading, start, end);
    b.heading_level = Some(level);
    b.content_range = Some(heading_content_range(builder.text, start, end));
    let text: String = subtree
        .iter()
        .filter_map(|(ev, _)| match ev {
            Event::Text(t) | Event::Code(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect();
    builder.headings.push(HeadingNode {
        level,
        text,
        source_range: ByteRange::new(start, end),
    });
    b
}

fn classify_paragraph(subtree: &[Ev], start: usize, end: usize) -> PendingBlock {
    let mut b = PendingBlock::new(BlockKind::Paragraph, start, end);
    let has_display_math = subtree
        .iter()
        .any(|(ev, _)| matches!(ev, Event::DisplayMath(_)));
    if has_display_math {
        return b.island(RawIslandType::MathBlock, "display math");
    }
    b.content_range = Some(ByteRange::new(start, end));
    b
}

fn classify_blockquote(text: &str, subtree: &[Ev], start: usize, end: usize) -> PendingBlock {
    let mut b = PendingBlock::new(BlockKind::Blockquote, start, end);
    let inner_starts = subtree
        .iter()
        .skip(1)
        .filter(|(ev, _)| matches!(ev, Event::Start(_)))
        .count();
    let single_line = !text[start..end].contains('\n');
    let single_paragraph = inner_starts == 1
        && subtree
            .iter()
            .skip(1)
            .any(|(ev, _)| matches!(ev, Event::Start(Tag::Paragraph)));
    if !(single_line && single_paragraph) {
        return b.island(
            RawIslandType::ComplexBlockquote,
            "multi-line or nested blockquote",
        );
    }
    // Single line: content begins after `>` and one optional space.
    let line = &text[start..end];
    let after_marker = line.find('>').map(|p| p + 1).unwrap_or(0);
    let mut content_start = start + after_marker;
    if text[content_start..end].starts_with(' ') {
        content_start += 1;
    }
    b.content_range = Some(ByteRange::new(content_start, end));
    b
}

fn classify_code(
    builder: &mut Builder,
    kind: CodeBlockKind,
    start: usize,
    end: usize,
) -> PendingBlock {
    let mut b = PendingBlock::new(BlockKind::FencedCode, start, end);
    let CodeBlockKind::Fenced(info) = kind else {
        return b.island(RawIslandType::UnknownExtension, "indented code block");
    };
    let text = builder.text;
    let first_line = text[start..end].lines().next().unwrap_or("");
    let indent_len = first_line.len() - first_line.trim_start().len();
    if indent_len > 0 {
        return b.island(
            RawIslandType::UnknownExtension,
            "indented fenced code block",
        );
    }
    let marker = first_line.chars().next().unwrap_or('`');
    let fence_len = first_line.chars().take_while(|c| *c == marker).count();
    b.fence_style = Some(CodeFenceStyle {
        marker,
        length: fence_len,
    });
    let lang = info.as_ref().trim();
    b.code_language = (!lang.is_empty()).then(|| lang.to_string());

    // Content spans from after the opening fence line to before the closing
    // fence line. An unclosed fence is preserved as an island (mutating it
    // structurally would add a fence the user never wrote).
    let slice = &text[start..end];
    let Some(open_nl) = slice.find('\n') else {
        return b.island(RawIslandType::MalformedRegion, "unclosed code fence");
    };
    let last_line_start = slice.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let last_line = slice[last_line_start..].trim_end();
    let closes = last_line.chars().take_while(|c| *c == marker).count() >= fence_len
        && last_line.chars().all(|c| c == marker);
    if !closes || last_line_start <= open_nl {
        return b.island(RawIslandType::MalformedRegion, "unclosed code fence");
    }
    b.content_range = Some(ByteRange::new(start + open_nl + 1, start + last_line_start));
    b
}

fn heading_level_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Content range of a heading: ATX -> text after the `#` run and one space,
/// trimmed of a CommonMark closing sequence; setext -> the first line.
fn heading_content_range(text: &str, start: usize, end: usize) -> ByteRange {
    let slice = &text[start..end];
    let first_line = slice.lines().next().unwrap_or("");
    let indent = first_line.len() - first_line.trim_start().len();
    let trimmed = &first_line[indent..];
    if trimmed.starts_with('#') {
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        let mut content_start = indent + hashes;
        if first_line[content_start..].starts_with(' ') {
            content_start += 1;
        }
        let mut content_end = first_line.trim_end().len();
        // Strip an ATX closing sequence (` ###`) when present.
        let body = &first_line[content_start..content_end];
        let closing = body.chars().rev().take_while(|c| *c == '#').count();
        if closing > 0 {
            let before = body.len() - closing;
            if before == 0 || body[..before].ends_with(' ') {
                content_end = content_start + body[..before].trim_end().len();
            }
        }
        ByteRange::new(
            start + content_start,
            start + content_end.max(content_start),
        )
    } else {
        // Setext heading: editable text is the first line.
        ByteRange::new(start, start + first_line.trim_end().len())
    }
}

/// Trims trailing `\n` / `\r\n` sequences from a block end offset.
pub(crate) fn trim_trailing_newlines(text: &str, mut end: usize) -> usize {
    let bytes = text.as_bytes();
    while end > 0 && (bytes[end - 1] == b'\n' || bytes[end - 1] == b'\r') {
        end -= 1;
    }
    end
}

/// Classifies a GFM table as `SimpleTable` (all cells plain text,
/// form-editable) or `ComplexTable` raw island (RFC-027).
fn classify_table(subtree: &[Ev], start: usize, end: usize) -> PendingBlock {
    // If any cell contains inline markup events, demote to ComplexTable island.
    let is_simple = subtree.iter().all(|(ev, _)| {
        !matches!(
            ev,
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough)
                | Event::Code(_)
                | Event::InlineHtml(_)
        )
    });
    if is_simple {
        PendingBlock::new(crate::block::BlockKind::SimpleTable, start, end)
    } else {
        PendingBlock::new(crate::block::BlockKind::ComplexTable, start, end)
            .island(RawIslandType::ComplexTable, "complex table")
    }
}
