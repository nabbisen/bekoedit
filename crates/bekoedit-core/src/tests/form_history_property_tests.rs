//! RFC-049 slice 1: N random Form edits, N Undos, the original bytes.

use bekoedit_markdown::{
    FormBlockEdit, FormEditCommand, TableAlignment, TableColumnPosition, TableRowDirection,
    TableRowPosition,
};

use super::form_history_tests::{MIXED, session};
use crate::session::DocumentSession;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_edit(s: &DocumentSession, rng: &mut Rng) -> Option<FormEditCommand> {
    let blocks = &s.index.blocks;
    if blocks.is_empty() {
        return None;
    }
    let block_id = blocks[rng.below(blocks.len())].block_id;
    let words = ["x", "日本語", "a | b", "line", "  spaced  "];
    let pick = |rng: &mut Rng| words[rng.below(words.len())].to_string();
    let edit = match rng.below(14) {
        0 => FormBlockEdit::ReplacePlainText { text: pick(rng) },
        1 => FormBlockEdit::SetHeadingLevel {
            level: 1 + rng.below(6) as u8,
        },
        2 => FormBlockEdit::ToggleTaskChecked {
            item_ordinal: 1 + rng.below(3) as u32,
            checked: rng.below(2) == 0,
        },
        3 => FormBlockEdit::ReplaceListItemText {
            item_ordinal: 1 + rng.below(3) as u32,
            text: pick(rng),
        },
        4 => FormBlockEdit::ReplaceCodeBlock {
            language: None,
            code: pick(rng),
        },
        5 => FormBlockEdit::ReplaceTableCell {
            row: rng.below(4),
            col: rng.below(3),
            text: pick(rng),
        },
        6 => FormBlockEdit::InsertTableRow {
            at: rng.below(4),
            position: if rng.below(2) == 0 {
                TableRowPosition::Above
            } else {
                TableRowPosition::Below
            },
        },
        7 => FormBlockEdit::DeleteTableRow { row: rng.below(4) },
        8 => FormBlockEdit::MoveTableRow {
            row: rng.below(4),
            direction: if rng.below(2) == 0 {
                TableRowDirection::Up
            } else {
                TableRowDirection::Down
            },
        },
        9 => FormBlockEdit::InsertTableColumn {
            col: rng.below(3),
            position: TableColumnPosition::Left,
        },
        10 => FormBlockEdit::DeleteTableColumn { col: rng.below(3) },
        11 => FormBlockEdit::SetTableColumnAlignment {
            col: rng.below(3),
            alignment: TableAlignment::Centre,
        },
        12 => FormBlockEdit::TidyTable,
        _ => FormBlockEdit::DeleteBlock,
    };
    Some(FormEditCommand {
        base_revision: s.revision,
        block_id,
        client_block_fingerprint: None,
        edit,
    })
}

#[test]
fn random_form_edits_undo_to_the_original_bytes_and_redo_to_the_edited_ones() {
    for seed in 1..=250u64 {
        for text in [MIXED.to_string(), MIXED.replace('\n', "\r\n")] {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut s = session(&text);
            let mut recorded = 0;
            for _ in 0..(1 + rng.below(12)) {
                let Some(command) = random_edit(&s, &mut rng) else {
                    break;
                };
                let before = s.canonical_text.clone();
                if s.apply_form_edit(&command).is_ok() && s.canonical_text != before {
                    recorded += 1;
                }
            }
            let edited = s.canonical_text.clone();
            for n in 1..=recorded {
                assert!(
                    s.undo_form().unwrap(),
                    "seed {seed}: undo {n} of {recorded}"
                );
            }
            assert_eq!(s.canonical_text, text, "seed {seed}: undone");
            assert!(!s.undo_form().unwrap(), "seed {seed}: nothing beneath");
            for n in 1..=recorded {
                assert!(
                    s.redo_form().unwrap(),
                    "seed {seed}: redo {n} of {recorded}"
                );
            }
            assert_eq!(s.canonical_text, edited, "seed {seed}: redone");
            assert!(!s.redo_form().unwrap(), "seed {seed}: nothing above");
        }
    }
}
