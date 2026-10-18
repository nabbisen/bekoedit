//! Task 064: an empty heading never panics the index, and Form Mode never
//! destroys a heading.

use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, FormEditError, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn index(text: &str) -> MarkdownIndex {
    MarkdownIndex::build(text, 1)
}

/// The first heading must be of `level`, with text `content`.
fn assert_heading(text: &str, level: u8, content: &str) {
    let index = index(text);
    let block = index
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Heading)
        .unwrap_or_else(|| panic!("no heading in {text:?}"));
    assert_eq!(block.heading_level, Some(level), "{text:?}");
    let range = block.content_range.expect("a heading has a content range");
    assert_eq!(&text[range.start..range.end], content, "{text:?}");
}

#[test]
fn an_empty_atx_heading_is_indexed_as_a_heading_with_empty_text() {
    for text in [
        "# \n",
        "# \nbody\n",
        "#  \n",
        "# \r\n",
        "# ",
        "#\n",
        "#\t\n",
        "# \t\n",
        "# #\n",
        "# # \n",
    ] {
        assert_heading(text, 1, "");
    }
    assert_heading("text\n\n## \n", 2, "");
}

#[test]
fn each_level_and_the_closing_sequence_index_to_empty_text() {
    for level in 1..=6u8 {
        let hashes = "#".repeat(level as usize);
        for text in [
            format!("{hashes} \n"),
            format!("{hashes}\n"),
            format!("{hashes} {hashes}\n"),
            format!("{hashes} {hashes} \n"),
            format!("{hashes} \r\n"),
            format!("   {hashes} \n"),
            format!("{hashes} "),
        ] {
            assert_heading(&text, level, "");
        }
    }
}

#[test]
fn a_populated_heading_still_indexes_as_before() {
    assert_heading("# Title\n", 1, "Title");
    assert_heading("## Sub ##\n", 2, "Sub");
    assert_heading("# Title #\r\n", 1, "Title");
    assert_heading("   ### x  \n", 3, "x");
    assert_heading("# 日本語 \n", 1, "日本語");
}

#[test]
fn indexing_changes_no_byte_of_the_text() {
    let text = String::from("# \n\nbody\n\n## \r\n");
    let before = text.clone();
    let _ = index(&text);
    assert_eq!(text, before);
}

// --- Form Mode ---

fn clear_heading(text: &str) -> Result<String, FormEditError> {
    let index = index(text);
    let block = index
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::Heading)
        .unwrap();
    let command = FormEditCommand {
        base_revision: 1,
        block_id: block.block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: String::new(),
        },
    };
    let patch = resolve_form_edit(text, &index, &command)?;
    let mut out = text.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    Ok(out)
}

#[test]
fn clearing_an_atx_heading_leaves_an_empty_heading_of_the_same_level() {
    for level in 1..=6u8 {
        let hashes = "#".repeat(level as usize);
        for nl in ["\n", "\r\n"] {
            let text = format!("{hashes} Title{nl}{nl}body{nl}");
            let cleared = clear_heading(&text).unwrap();
            // The hashes and the one space stay, so the field can be typed in again.
            assert_eq!(cleared, format!("{hashes} {nl}{nl}body{nl}"));
            assert_heading(&cleared, level, "");
        }
    }
}

#[test]
fn clearing_a_heading_without_a_final_newline() {
    let cleared = clear_heading("# Title").unwrap();
    assert_eq!(cleared, "# ");
    assert_heading(&cleared, 1, "");
}

#[test]
fn typing_into_a_cleared_heading_works_again() {
    let cleared = clear_heading("## Title\n").unwrap();
    let index = index(&cleared);
    let command = FormEditCommand {
        base_revision: 1,
        block_id: index.blocks[0].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: "Again".into(),
        },
    };
    let patch = resolve_form_edit(&cleared, &index, &command).unwrap();
    let mut out = cleared.clone();
    apply_patch(&mut out, 1, &patch).unwrap();
    assert_eq!(out, "## Again\n");
}

#[test]
fn clearing_a_setext_heading_is_refused_and_changes_nothing() {
    for text in [
        "Title\n=====\n\nx\n",
        "Title\r\n=====\r\n\r\nx\r\n",
        "Sub\n---\n\nx\n",
    ] {
        let error = clear_heading(text).unwrap_err();
        assert!(
            matches!(error, FormEditError::UnsupportedEditOperation { .. }),
            "{text:?}: {error:?}"
        );
    }
}

#[test]
fn editing_a_setext_heading_to_other_text_still_works() {
    let text = "Title\n=====\n";
    let index = index(text);
    let command = FormEditCommand {
        base_revision: 1,
        block_id: index.blocks[0].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText { text: "New".into() },
    };
    let patch = resolve_form_edit(text, &index, &command).unwrap();
    let mut out = text.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    assert_eq!(out, "New\n=====\n");
}

// --- property: the index never panics ---

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Weighted toward the characters that make Markdown structure.
const ALPHABET: &[&str] = &[
    "#", "#", "#", " ", " ", " ", "\t", "\r", "\n", "\n", "\n", "=", "-", "|", "`", ">", "*", "日",
    "a", "[", "]", "(", ")", "1", ".", "~", "_", "!", ":",
];

fn random_document(seed: u64) -> String {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let len = 1 + (rng.next() % 24) as usize;
    (0..len)
        .map(|_| ALPHABET[(rng.next() % ALPHABET.len() as u64) as usize])
        .collect()
}

#[test]
fn the_index_never_panics_on_random_short_documents() {
    for seed in 1..=4000u64 {
        let text = random_document(seed);
        let outcome = std::panic::catch_unwind(|| {
            let index = MarkdownIndex::build(&text, 1);
            // Every range it reports must lie inside the text, on boundaries.
            for block in &index.blocks {
                for range in [Some(block.source_range), block.content_range]
                    .into_iter()
                    .flatten()
                {
                    assert!(
                        text.get(range.start..range.end).is_some(),
                        "bad range {range:?}"
                    );
                }
            }
        });
        assert!(
            outcome.is_ok(),
            "MarkdownIndex::build panicked: seed {seed}, text {text:?}"
        );
    }
}
