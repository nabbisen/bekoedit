//! RFC-049 slice 1: Undo and Redo of committed Form Mode edits, headless.

use bekoedit_markdown::{
    BlockKind, FormBlockDisplay, FormBlockEdit, FormEditCommand, InlineFormat, TableAlignment,
    TableColumnDirection, TableColumnPosition, TableRowDirection, TableRowPosition,
};

use crate::session::{DocumentSession, SessionError};

pub(super) const MIXED: &str = "# Title\n\nPara 日本語 text\n\n- [ ] one\n- [x] two\n\n\
| a | b |\n| --- | --- |\n| 1 | 日本 |\n| 3 | 4 |\n\n```rust\nfn x() {}\n```\n\nLast para\n";

pub(super) fn session(text: &str) -> DocumentSession {
    DocumentSession::from_text(1, "/x.md".into(), text.into())
}

fn block(s: &DocumentSession, kind: BlockKind) -> bekoedit_markdown::BlockId {
    s.index
        .blocks
        .iter()
        .find(|b| b.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} block"))
        .block_id
}

pub(super) fn cmd(s: &DocumentSession, kind: BlockKind, edit: FormBlockEdit) -> FormEditCommand {
    FormEditCommand {
        base_revision: s.revision,
        block_id: block(s, kind),
        client_block_fingerprint: None,
        edit,
    }
}

fn first_item_ordinal(s: &DocumentSession) -> u32 {
    s.form_projection()
        .blocks
        .iter()
        .find_map(|b| match &b.display {
            FormBlockDisplay::List { items, .. } => items.first().map(|i| i.ordinal),
            _ => None,
        })
        .unwrap()
}

/// Every Form edit kind, each against `MIXED` (or its CRLF form).
fn edits(s: &DocumentSession) -> Vec<(&'static str, FormEditCommand)> {
    use BlockKind::*;
    let table = |edit| cmd(s, SimpleTable, edit);
    let ord = first_item_ordinal(s);
    vec![
        (
            "text commit",
            cmd(
                s,
                Paragraph,
                FormBlockEdit::ReplacePlainText {
                    text: "新しい text".into(),
                },
            ),
        ),
        (
            "heading level",
            cmd(s, Heading, FormBlockEdit::SetHeadingLevel { level: 3 }),
        ),
        (
            "toggle",
            cmd(
                s,
                TaskList,
                FormBlockEdit::ToggleTaskChecked {
                    item_ordinal: ord,
                    checked: true,
                },
            ),
        ),
        (
            "list item",
            cmd(
                s,
                TaskList,
                FormBlockEdit::ReplaceListItemText {
                    item_ordinal: ord,
                    text: "uno".into(),
                },
            ),
        ),
        (
            "code",
            cmd(
                s,
                FencedCode,
                FormBlockEdit::ReplaceCodeBlock {
                    language: Some("py".into()),
                    code: "pass\n".into(),
                },
            ),
        ),
        (
            "cell",
            table(FormBlockEdit::ReplaceTableCell {
                row: 1,
                col: 1,
                text: "セル".into(),
            }),
        ),
        (
            "row insert",
            table(FormBlockEdit::InsertTableRow {
                at: 1,
                position: TableRowPosition::Below,
            }),
        ),
        (
            "row delete",
            table(FormBlockEdit::DeleteTableRow { row: 1 }),
        ),
        (
            "row move",
            table(FormBlockEdit::MoveTableRow {
                row: 1,
                direction: TableRowDirection::Down,
            }),
        ),
        (
            "column insert",
            table(FormBlockEdit::InsertTableColumn {
                col: 0,
                position: TableColumnPosition::Right,
            }),
        ),
        (
            "column delete",
            table(FormBlockEdit::DeleteTableColumn { col: 1 }),
        ),
        (
            "column move",
            table(FormBlockEdit::MoveTableColumn {
                col: 0,
                direction: TableColumnDirection::Right,
            }),
        ),
        (
            "alignment",
            table(FormBlockEdit::SetTableColumnAlignment {
                col: 0,
                alignment: TableAlignment::Right,
            }),
        ),
        ("tidy", table(FormBlockEdit::TidyTable)),
        (
            "block delete",
            cmd(s, Paragraph, FormBlockEdit::DeleteBlock),
        ),
        (
            "inline toggle",
            cmd(
                s,
                Paragraph,
                FormBlockEdit::ToggleInline {
                    current_text: "Para 日本語 text".into(),
                    kind: InlineFormat::Bold,
                    utf16_start: 0,
                    utf16_len: 4,
                    link_url: None,
                },
            ),
        ),
    ]
}

fn round_trip_every_kind(text: &str) {
    let probe = session(text);
    for (name, _) in edits(&probe) {
        let mut s = session(text);
        let command = edits(&s).into_iter().find(|(n, _)| *n == name).unwrap().1;
        s.apply_form_edit(&command)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let edited = s.canonical_text.clone();
        assert_ne!(edited, text, "{name} changed nothing");
        assert!(s.undo_form().unwrap(), "{name}");
        assert_eq!(s.canonical_text, text, "{name}: undo");
        assert!(s.redo_form().unwrap(), "{name}");
        assert_eq!(s.canonical_text, edited, "{name}: redo");
    }
}

#[test]
fn each_form_edit_kind_undoes_and_redoes_to_the_exact_bytes() {
    round_trip_every_kind(MIXED);
}

#[test]
fn the_same_with_crlf_line_endings() {
    round_trip_every_kind(&MIXED.replace('\n', "\r\n"));
}

pub(super) fn paragraph_edit(s: &DocumentSession, text: &str) -> FormEditCommand {
    cmd(
        s,
        BlockKind::Paragraph,
        FormBlockEdit::ReplacePlainText { text: text.into() },
    )
}

#[test]
fn undo_twice_then_redo_twice_across_two_different_edits() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "first");
    s.apply_form_edit(&c).unwrap();
    let after_first = s.canonical_text.clone();
    let c = cmd(
        &s,
        BlockKind::Heading,
        FormBlockEdit::SetHeadingLevel { level: 2 },
    );
    s.apply_form_edit(&c).unwrap();
    let after_second = s.canonical_text.clone();

    assert!(s.undo_form().unwrap());
    assert_eq!(s.canonical_text, after_first);
    assert!(s.undo_form().unwrap());
    assert_eq!(s.canonical_text, MIXED);
    assert!(!s.undo_form().unwrap());
    assert!(s.redo_form().unwrap());
    assert_eq!(s.canonical_text, after_first);
    assert!(s.redo_form().unwrap());
    assert_eq!(s.canonical_text, after_second);
    assert!(!s.redo_form().unwrap());
}

#[test]
fn a_text_mode_edit_makes_undo_refuse_and_empties_both_stacks() {
    let mut s = session(MIXED);
    for text in ["form", "form again"] {
        let c = paragraph_edit(&s, text);
        s.apply_form_edit(&c).unwrap();
    }
    s.undo_form().unwrap();
    assert!(s.can_undo_form() && s.can_redo_form());
    let rev = s.revision;
    s.apply_editor_text(rev, "# typed in Text Mode\n").unwrap();
    let typed = s.canonical_text.clone();

    assert_eq!(s.undo_form().unwrap_err(), SessionError::FormHistoryStale);
    assert_eq!(s.canonical_text, typed);
    assert!(!s.can_undo_form() && !s.can_redo_form());
}

#[test]
fn a_stale_redo_refuses_too() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "form");
    s.apply_form_edit(&c).unwrap();
    s.undo_form().unwrap();
    let rev = s.revision;
    s.apply_text_snapshot(rev, "# other\n".into()).unwrap();
    assert_eq!(s.redo_form().unwrap_err(), SessionError::FormHistoryStale);
    assert_eq!(s.canonical_text, "# other\n");
    assert!(!s.can_undo_form() && !s.can_redo_form());
}

#[test]
fn a_restored_snapshot_makes_undo_refuse() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "form");
    s.apply_form_edit(&c).unwrap();
    s.apply_restored_snapshot("# restored\n".into());
    assert_eq!(s.undo_form().unwrap_err(), SessionError::FormHistoryStale);
    assert_eq!(s.canonical_text, "# restored\n");
}

#[test]
fn a_new_edit_after_a_text_edit_starts_a_fresh_history() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "one");
    s.apply_form_edit(&c).unwrap();
    let rev = s.revision;
    s.apply_editor_text(rev, "# t\n\npara\n").unwrap();
    let before = s.canonical_text.clone();
    let c = paragraph_edit(&s, "two");
    s.apply_form_edit(&c).unwrap();
    assert!(s.undo_form().unwrap());
    assert_eq!(s.canonical_text, before);
    assert!(
        !s.undo_form().unwrap(),
        "the stale first record was dropped"
    );
}

#[test]
fn a_no_op_edit_is_not_recorded_and_does_not_clear_redo() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "changed");
    s.apply_form_edit(&c).unwrap();
    s.undo_form().unwrap();
    let c = paragraph_edit(&s, "Para 日本語 text");
    let rev = s.revision;
    s.apply_form_edit(&c).unwrap();
    assert_eq!(s.canonical_text, MIXED);
    assert_eq!(s.revision, rev + 1, "it is still a mutation");
    assert!(s.can_redo_form());
    assert!(!s.can_undo_form(), "nothing was recorded");
    assert!(s.redo_form().unwrap(), "Redo still fits after the no-op");
    assert!(s.undo_form().unwrap(), "and so does the Undo it makes");
    assert_eq!(s.canonical_text, MIXED);
}

#[test]
fn a_no_op_edit_does_not_break_the_undo_beneath_it() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "changed");
    s.apply_form_edit(&c).unwrap();
    let c = paragraph_edit(&s, "changed");
    s.apply_form_edit(&c).unwrap();
    assert!(s.undo_form().unwrap());
    assert_eq!(s.canonical_text, MIXED);
}

#[test]
fn a_new_edit_after_an_undo_clears_redo() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "a");
    s.apply_form_edit(&c).unwrap();
    s.undo_form().unwrap();
    assert!(s.can_redo_form());
    let c = paragraph_edit(&s, "b");
    s.apply_form_edit(&c).unwrap();
    assert!(!s.can_redo_form());
}

#[test]
fn a_refused_edit_records_nothing_and_leaves_both_stacks() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "a");
    s.apply_form_edit(&c).unwrap();
    let c = paragraph_edit(&s, "b");
    s.apply_form_edit(&c).unwrap();
    s.undo_form().unwrap();
    let mut stale = paragraph_edit(&s, "c");
    stale.base_revision = 1;
    assert!(s.apply_form_edit(&stale).is_err());
    assert!(s.can_undo_form() && s.can_redo_form());
    assert!(s.redo_form().unwrap());
    assert!(s.undo_form().unwrap() && s.undo_form().unwrap());
    assert_eq!(s.canonical_text, MIXED);
}

#[test]
fn the_history_is_bounded_to_100_records() {
    let mut s = session(MIXED);
    let mut after_first = String::new();
    for i in 1..=101 {
        let c = paragraph_edit(&s, &format!("edit {i}"));
        s.apply_form_edit(&c).unwrap();
        if i == 1 {
            after_first = s.canonical_text.clone();
        }
    }
    for n in 1..=100 {
        assert!(s.undo_form().unwrap(), "undo {n}");
    }
    assert!(!s.undo_form().unwrap(), "the 101st has nothing left");
    assert_eq!(s.canonical_text, after_first);
}

#[test]
fn every_session_mutator_increments_the_revision() {
    let mut s = session(MIXED);
    let mut last = s.revision;
    let mut step = |s: &DocumentSession| {
        assert_eq!(s.revision, last + 1);
        last = s.revision;
    };
    let rev = s.revision;
    s.apply_text_snapshot(rev, "# a\n\npara\n".into()).unwrap();
    step(&s);
    let rev = s.revision;
    s.apply_editor_text(rev, "# b\n\npara\n").unwrap();
    step(&s);
    let c = paragraph_edit(&s, "formed");
    s.apply_form_edit(&c).unwrap();
    step(&s);
    s.apply_restored_snapshot("# c\n\npara\n".into());
    step(&s);
    let c = paragraph_edit(&s, "formed again");
    s.apply_form_edit(&c).unwrap();
    step(&s);
    s.undo_form().unwrap();
    step(&s);
    s.redo_form().unwrap();
    step(&s);
}

#[test]
fn clearing_the_history_empties_both_stacks() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "a");
    s.apply_form_edit(&c).unwrap();
    s.undo_form().unwrap();
    s.clear_form_history();
    assert!(!s.can_undo_form() && !s.can_redo_form());
}

#[test]
fn history_does_not_change_session_equality() {
    let mut s = session(MIXED);
    let c = paragraph_edit(&s, "a");
    s.apply_form_edit(&c).unwrap();
    let mut without = s.clone();
    without.clear_form_history();
    assert!(s.can_undo_form() && !without.can_undo_form());
    assert_eq!(
        without, s,
        "equality compares the document, not the history"
    );
}
