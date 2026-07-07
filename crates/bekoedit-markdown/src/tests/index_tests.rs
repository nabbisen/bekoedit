//! RFC-013 acceptance criteria: heading outline, safe block byte ranges,
//! raw island detection, and reparse validity.

use crate::block::{BlockKind, EditablePolicy};
use crate::index::MarkdownIndex;
use crate::island::RawIslandType;
use crate::trivia::LineEnding;

const DOC: &str = "---\ntitle: Example\n---\n\n# Title\n\nIntro paragraph.\n\n## Goals\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n\n<div>html</div>\n";

#[test]
fn front_matter_becomes_raw_island() {
    let idx = MarkdownIndex::build(DOC, 1);
    let fm = &idx.blocks[0];
    assert_eq!(fm.kind, BlockKind::FrontMatter);
    assert_eq!(fm.editable_policy, EditablePolicy::RawIslandOnly);
    assert_eq!(
        &DOC[fm.source_range.start..fm.source_range.end],
        "---\ntitle: Example\n---"
    );
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::FrontMatter)
    );
}

#[test]
fn headings_feed_the_outline() {
    let idx = MarkdownIndex::build(DOC, 1);
    let levels: Vec<(u8, &str)> = idx
        .headings
        .iter()
        .map(|h| (h.level, h.text.as_str()))
        .collect();
    assert_eq!(levels, vec![(1, "Title"), (2, "Goals")]);
}

#[test]
fn safe_blocks_map_to_exact_source_ranges() {
    let idx = MarkdownIndex::build(DOC, 1);
    let para = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .expect("paragraph");
    assert_eq!(
        &DOC[para.source_range.start..para.source_range.end],
        "Intro paragraph."
    );
    let heading = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Heading)
        .expect("heading");
    let content = heading.content_range.expect("heading content");
    assert_eq!(&DOC[content.start..content.end], "Title");
}

#[test]
fn html_block_becomes_island() {
    let idx = MarkdownIndex::build(DOC, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::HtmlBlock)
    );
}

#[test]
fn code_fence_style_is_captured() {
    let idx = MarkdownIndex::build(DOC, 1);
    let code = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::FencedCode)
        .expect("code block");
    let style = code.trivia.code_fence_style.expect("fence style");
    assert_eq!(style.marker, '`');
    assert_eq!(style.length, 3);
    assert_eq!(code.code_language.as_deref(), Some("rust"));
    let content = code.content_range.expect("code content");
    assert_eq!(&DOC[content.start..content.end], "fn main() {}\n");
}

#[test]
fn tilde_fences_are_preserved_as_style() {
    let doc = "~~~~python\nprint('hi')\n~~~~\n";
    let idx = MarkdownIndex::build(doc, 1);
    let code = &idx.blocks[0];
    let style = code.trivia.code_fence_style.expect("fence style");
    assert_eq!(style.marker, '~');
    assert_eq!(style.length, 4);
}

#[test]
fn unclosed_fence_is_a_malformed_island() {
    let doc = "```\nno closing fence\n";
    let idx = MarkdownIndex::build(doc, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::MalformedRegion)
    );
}

#[test]
fn task_list_items_are_indexed_with_checkbox_state() {
    let doc = "- [ ] todo\n- [x] done\n";
    let idx = MarkdownIndex::build(doc, 1);
    let list = &idx.blocks[0];
    assert_eq!(list.kind, BlockKind::TaskList);
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[0].task_checked, Some(false));
    assert_eq!(list.items[1].task_checked, Some(true));
    let c0 = list.items[0].content_range;
    assert_eq!(&doc[c0.start..c0.end], "todo");
}

#[test]
fn nested_lists_downgrade_to_complex_island() {
    let doc = "- parent\n  - child\n";
    let idx = MarkdownIndex::build(doc, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::ComplexList)
    );
}

#[test]
fn simple_table_is_form_editable_not_a_complex_island() {
    // RFC-027: plain-cell tables become SimpleTable (form-editable).
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == crate::block::BlockKind::SimpleTable);
    assert!(
        table.is_some(),
        "simple table must produce a SimpleTable block"
    );
    assert!(
        idx.raw_islands
            .iter()
            .all(|i| i.island_type != RawIslandType::ComplexTable),
        "simple table must not appear as a ComplexTable island"
    );
}

#[test]
fn table_with_bold_cells_is_complex_island() {
    // A table containing **bold** remains a ComplexTable raw island.
    let doc = "| **Name** | Score |\n|----------|-------|\n| Alice | 42 |\n";
    let idx = MarkdownIndex::build(doc, 1);
    assert!(
        idx.raw_islands
            .iter()
            .any(|i| i.island_type == RawIslandType::ComplexTable),
        "table with inline markup must remain a ComplexTable island"
    );
}

#[test]
fn reparse_after_mutation_produces_valid_index() {
    let mut text = String::from("# A\n\nbody\n");
    let idx1 = MarkdownIndex::build(&text, 1);
    assert_eq!(idx1.document_revision, 1);
    text = text.replace("body", "new body");
    let idx2 = MarkdownIndex::build(&text, 2);
    assert_eq!(idx2.document_revision, 2);
    let para = idx2
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .unwrap();
    assert_eq!(
        &text[para.source_range.start..para.source_range.end],
        "new body"
    );
}

#[test]
fn block_resolution_requires_matching_identity() {
    let idx = MarkdownIndex::build("# A\n\nbody\n", 1);
    let real = idx.blocks[1].block_id;
    assert!(idx.resolve_block(&real).is_some());
    let mut stale = real;
    stale.fingerprint.content_hash ^= 1;
    assert!(idx.resolve_block(&stale).is_none());
}

#[test]
fn line_ending_detection() {
    assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
    assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
    assert_eq!(LineEnding::detect("a\nb\r\n"), LineEnding::Mixed);
}

#[test]
fn crlf_documents_index_correctly() {
    let doc = "# Title\r\n\r\nParagraph text.\r\n";
    let idx = MarkdownIndex::build(doc, 1);
    let para = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Paragraph)
        .unwrap();
    assert_eq!(
        &doc[para.source_range.start..para.source_range.end],
        "Paragraph text."
    );
}

#[test]
fn japanese_and_emoji_ranges_are_char_safe() {
    let doc = "# 見出し 🎌\n\n日本語の段落です。絵文字😀も含む。\n";
    let idx = MarkdownIndex::build(doc, 1);
    for block in &idx.blocks {
        assert!(block.source_range.validate(doc).is_ok());
        if let Some(c) = block.content_range {
            assert!(c.validate(doc).is_ok());
        }
    }
    assert_eq!(idx.headings[0].text, "見出し 🎌");
}
