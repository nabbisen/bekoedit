// RFC-048 slice 5: Tidy table. Split out per the ELOC guideline.

use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn tidy(doc: &str) -> String {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"));
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: table.block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::TidyTable,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).unwrap_or_else(|e| panic!("{doc:?}: {e:?}"));
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    out
}

fn is_tidy(doc: &str) -> bool {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap();
    crate::form::table_is_tidy(&doc[table.source_range.start..table.source_range.end])
}

#[test]
fn a_ragged_table_is_aligned_to_its_widest_cell() {
    assert_eq!(
        tidy("| a | long |\n|---|---|\n| xxxxx | b |\n"),
        "| a     | long |\n| ----- | ---- |\n| xxxxx | b    |\n"
    );
}

#[test]
fn a_narrow_table_still_gets_the_three_dash_minimum() {
    assert_eq!(
        tidy("|a|b|\n|-|-|\n|1|2|\n"),
        "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n"
    );
}

#[test]
fn japanese_text_counts_as_two_columns_per_character() {
    // 名前 is 4 columns wide, so `Bob` (3) pads by one and `20` (2) by two.
    assert_eq!(
        tidy("| 名前 | 年齢 |\n|---|---|\n| 太郎 | 20 |\n| Bob | 30 |\n"),
        "| 名前 | 年齢 |\n| ---- | ---- |\n| 太郎 | 20   |\n| Bob  | 30   |\n"
    );
}

#[test]
fn mixed_width_text_pads_by_display_width_not_bytes() {
    // `名` is 3 bytes and 2 columns, and the column is 3 wide (the minimum):
    // by display width it pads by one, by byte length it would pad by none.
    assert_eq!(
        tidy("| 名 | abcd |\n|---|---|\n| x | y |\n"),
        "| 名  | abcd |\n| --- | ---- |\n| x   | y    |\n"
    );
}

#[test]
fn every_alignment_colon_is_kept() {
    assert_eq!(
        tidy("| a | b | c | d |\n|:--|:-:|--:|---|\n| 1 | 2 | 3 | 4 |\n"),
        "| a   | b   | c   | d   |\n| :-- | :-: | --: | --- |\n| 1   | 2   | 3   | 4   |\n"
    );
}

#[test]
fn an_escaped_pipe_and_its_text_are_untouched() {
    assert_eq!(
        tidy("| a | x \\| y |\n|---|---|\n| 1 | 2 |\n"),
        "| a   | x \\| y |\n| --- | ------ |\n| 1   | 2      |\n"
    );
}

#[test]
fn a_formatted_cell_keeps_its_markup() {
    assert_eq!(
        tidy("| **a** | b |\n|---|---|\n| 1 | 2 |\n"),
        "| **a** | b   |\n| ----- | --- |\n| 1     | 2   |\n"
    );
}

#[test]
fn crlf_line_endings_stay() {
    assert_eq!(
        tidy("| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n"),
        "| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n"
    );
}

#[test]
fn no_final_newline_stays_absent() {
    assert_eq!(
        tidy("| a | b |\n|---|---|\n| 1 | 2 |"),
        "| a   | b   |\n| --- | --- |\n| 1   | 2   |"
    );
}

#[test]
fn a_table_with_no_outer_pipes_keeps_that_style() {
    assert_eq!(
        tidy("a | b\n--|--\n1 | 2\n"),
        "a   | b\n--- | ---\n1   | 2\n"
    );
}

#[test]
fn a_short_row_is_not_padded_with_new_cells() {
    assert_eq!(
        tidy("| a | b |\n|---|---|\n| 1 |\n"),
        "| a   | b   |\n| --- | --- |\n| 1   |\n"
    );
}

#[test]
fn a_long_rows_extra_cell_is_left_exactly_as_written() {
    assert_eq!(
        tidy("| a | b |\n|---|---|\n| 1 | 2 |  9  |\n"),
        "| a   | b   |\n| --- | --- |\n| 1   | 2   |  9  |\n"
    );
}

#[test]
fn text_before_and_after_the_table_is_untouched() {
    assert_eq!(
        tidy("intro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nafter\n"),
        "intro\n\n| a   | b   |\n| --- | --- |\n| 1   | 2   |\n\nafter\n"
    );
}

#[test]
fn tidy_is_idempotent_over_every_shape_above() {
    for doc in [
        "| a | long |\n|---|---|\n| xxxxx | b |\n",
        "|a|b|\n|-|-|\n|1|2|\n",
        "| 名前 | 年齢 |\n|---|---|\n| 太郎 | 20 |\n| Bob | 30 |\n",
        "| a | b | c | d |\n|:--|:-:|--:|---|\n| 1 | 2 | 3 | 4 |\n",
        "| a | x \\| y |\n|---|---|\n| 1 | 2 |\n",
        "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n",
        "| a | b |\n|---|---|\n| 1 | 2 |",
        "a | b\n--|--\n1 | 2\n",
        "| a | b |\n|---|---|\n| 1 |\n",
        "| a | b |\n|---|---|\n| 1 | 2 |  9  |\n",
    ] {
        let once = tidy(doc);
        assert_eq!(tidy(&once), once, "{doc:?} must tidy to a fixed point");
        assert!(is_tidy(&once), "{once:?} must report itself tidy");
    }
}

#[test]
fn an_already_tidy_table_is_reported_tidy_and_an_untidy_one_is_not() {
    assert!(is_tidy("| a   | b   |\n| --- | --- |\n| 1   | 2   |\n"));
    assert!(!is_tidy("| a | b |\n|---|---|\n| 1 | 2 |\n"));
}
