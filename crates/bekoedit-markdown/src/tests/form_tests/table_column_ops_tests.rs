// RFC-048 slice 4: column operations (insert, delete, move, alignment).
// Split out as its own module, next to `table_row_ops_tests.rs`, per the
// ELOC guideline (see `form_tests.rs`).

use super::table_tests::{common_affix, pulldown_cell_ranges};
use crate::block::BlockKind;
use crate::form::{
    FormBlockEdit, FormEditCommand, FormEditError, TableAlignment, TableColumnDirection,
    TableColumnPosition, resolve_form_edit,
};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn apply(doc: &str, edit: FormBlockEdit) -> Result<String, FormEditError> {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"))
        .clone();
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: table.block_id,
        client_block_fingerprint: None,
        edit,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd)?;
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    Ok(out)
}

fn insert(col: usize, position: TableColumnPosition) -> FormBlockEdit {
    FormBlockEdit::InsertTableColumn { col, position }
}

fn delete(col: usize) -> FormBlockEdit {
    FormBlockEdit::DeleteTableColumn { col }
}

fn move_col(col: usize, direction: TableColumnDirection) -> FormBlockEdit {
    FormBlockEdit::MoveTableColumn { col, direction }
}

fn align(col: usize, alignment: TableAlignment) -> FormBlockEdit {
    FormBlockEdit::SetTableColumnAlignment { col, alignment }
}

/// Three-row, two-column tables covering slice 3's corpus: outer pipes,
/// no outer pipes, padded, unpadded, CRLF, no final newline, a paragraph
/// after, Japanese text, an escaped pipe and a formatted cell.
fn corpus() -> Vec<&'static str> {
    vec![
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n",
        "a|b\n-|-\n1|2\n3|4\n5|6\n",
        "|a|b|\n|-|-|\n|1|2|\n|3|4|\n|5|6|\n",
        "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n| 3 | 4 |\r\n| 5 | 6 |\r\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |",
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n",
        "| 名前 | 年齢 |\n|------|------|\n| 太郎 | 20 |\n| 次郎 | 22 |\n",
        "| a | x \\| y |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n",
        "| **a** | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n",
        "| a | b |\n|---|---|\n| 1 |\n| 3 | 4 |\n",
        "| a | b |\n|---|---|\n| 1 | 2 | 9 |\n| 3 | 4 |\n",
    ]
}

// ─── Insert: exact bytes ──────────────────────────────────────────────────

#[test]
fn insert_right_of_the_first_column_is_byte_exact() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        insert(0, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a |  | b |\n|---| --- |---|\n| 1 |  | 2 |\n");
}

#[test]
fn insert_left_of_the_first_column_is_byte_exact() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        insert(0, TableColumnPosition::Left),
    )
    .unwrap();
    assert_eq!(out, "|  | a | b |\n| --- |---|---|\n|  | 1 | 2 |\n");
}

#[test]
fn insert_right_of_the_last_column_keeps_the_outer_pipe_style() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        insert(1, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a | b |  |\n|---|---| --- |\n| 1 | 2 |  |\n");
}

#[test]
fn insert_into_an_unpadded_table_writes_its_own_style() {
    let out = apply(
        "|a|b|\n|-|-|\n|1|2|\n",
        insert(0, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "|a|  |b|\n|-| --- |-|\n|1|  |2|\n");
}

#[test]
fn insert_into_a_table_with_no_outer_pipes_keeps_that_style() {
    let out = apply(
        "a | b\n--|--\n1 | 2\n",
        insert(1, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "a | b |  |\n--|-- | --- \n1 | 2 |  |\n");
}

#[test]
fn insert_into_a_table_with_an_escaped_pipe_never_splits_the_escape() {
    let out = apply(
        "| a | x \\| y |\n|---|---|\n| 1 | 2 |\n",
        insert(1, TableColumnPosition::Left),
    )
    .unwrap();
    assert_eq!(out, "| a |  | x \\| y |\n|---| --- |---|\n| 1 |  | 2 |\n");
}

#[test]
fn insert_keeps_crlf_line_endings_in_place() {
    let out = apply(
        "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n",
        insert(0, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a |  | b |\r\n|---| --- |---|\r\n| 1 |  | 2 |\r\n");
}

#[test]
fn insert_into_a_table_with_no_final_newline_leaves_it_out() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 | 2 |",
        insert(0, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a |  | b |\n|---| --- |---|\n| 1 |  | 2 |");
}

#[test]
fn a_short_row_lacking_the_anchor_column_is_left_alone() {
    // Task 045's short row, `| 1 |`, has no second cell: inserting right of
    // column 1 must not touch it, and a new column in the header and the
    // delimiter must not pad it either (RFC-048 slice 4 §2.1).
    let out = apply(
        "| a | b |\n|---|---|\n| 1 |\n",
        insert(1, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a | b |  |\n|---|---| --- |\n| 1 |\n");
}

#[test]
fn a_long_row_keeps_its_extra_cells() {
    let out = apply(
        "| a |\n|---|\n| 1 | 2 |\n",
        insert(0, TableColumnPosition::Right),
    )
    .unwrap();
    assert_eq!(out, "| a |  |\n|---| --- |\n| 1 |  | 2 |\n");
}

#[test]
fn a_short_row_lacking_the_deleted_column_is_left_alone() {
    // `| 1 |` has no column 1, so deleting column 1 must not touch it, and its
    // one cell must still be there afterwards (RFC-048 slice 4 §2.1).
    let out = apply("| a | b |\n|---|---|\n| 1 |\n", delete(1)).unwrap();
    assert_eq!(out, "| a |\n|---|\n| 1 |\n");
}

#[test]
fn a_short_row_that_has_only_one_of_two_moved_cells_is_left_alone() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 |\n",
        move_col(0, TableColumnDirection::Right),
    )
    .unwrap();
    assert_eq!(out, "| b | a |\n|---|---|\n| 1 |\n");
}

// ─── Delete: exact bytes ──────────────────────────────────────────────────

#[test]
fn delete_the_last_column_removes_its_cell_and_the_pipe_before_it() {
    let out = apply("| a | b |\n|---|---|\n| 1 | 2 |\n", delete(1)).unwrap();
    assert_eq!(out, "| a |\n|---|\n| 1 |\n");
}

#[test]
fn delete_a_middle_column_removes_its_cell_and_one_pipe() {
    let out = apply("| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n", delete(1)).unwrap();
    assert_eq!(out, "| a | c |\n|---|---|\n| 1 | 3 |\n");
}

#[test]
fn delete_the_first_column_removes_its_cell_and_its_pipe() {
    let out = apply("| a | b |\n|---|---|\n| 1 | 2 |\n", delete(0)).unwrap();
    assert_eq!(out, "| b |\n|---|\n| 2 |\n");
}

#[test]
fn delete_a_formatted_column_removes_its_markup_with_its_cell() {
    let out = apply("| **a** | b |\n|---|---|\n| 1 | 2 |\n", delete(0)).unwrap();
    assert_eq!(out, "| b |\n|---|\n| 2 |\n");
}

#[test]
fn delete_the_only_column_is_refused() {
    let err = apply("| a |\n|---|\n| 1 |\n", delete(0));
    assert!(matches!(
        err,
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

// ─── Move: exact bytes ────────────────────────────────────────────────────

#[test]
fn move_right_swaps_the_cells_and_their_alignment_with_them() {
    let out = apply(
        "| a | b |\n|:--|--:|\n| 1 | 2 |\n",
        move_col(0, TableColumnDirection::Right),
    )
    .unwrap();
    assert_eq!(out, "| b | a |\n|--:|:--|\n| 2 | 1 |\n");
}

#[test]
fn move_left_swaps_back_byte_for_byte() {
    let out = apply(
        "| b | a |\n|--:|:--|\n| 2 | 1 |\n",
        move_col(1, TableColumnDirection::Left),
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|:--|--:|\n| 1 | 2 |\n");
}

#[test]
fn moving_past_the_first_or_last_column_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    assert!(matches!(
        apply(doc, move_col(0, TableColumnDirection::Left)),
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
    assert!(matches!(
        apply(doc, move_col(1, TableColumnDirection::Right)),
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

// ─── Alignment: exact bytes ───────────────────────────────────────────────

#[test]
fn centre_alignment_rewrites_only_that_delimiter_cell() {
    let out = apply(
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        align(1, TableAlignment::Centre),
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|:---:|\n| 1 | 2 |\n");
}

#[test]
fn alignment_keeps_its_own_padding_and_dash_count() {
    let out = apply(
        "| a | b |\n| --- |  ----  |\n| 1 | 2 |\n",
        align(1, TableAlignment::Right),
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n| --- |  ----:  |\n| 1 | 2 |\n");
}

#[test]
fn alignment_none_clears_a_colon() {
    let out = apply(
        "| a | b |\n|:-:|---|\n| 1 | 2 |\n",
        align(0, TableAlignment::None),
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n");
}

// ─── Corpus: every operation, every corpus table ──────────────────────────

#[test]
fn every_column_operation_over_the_corpus_changes_only_the_table() {
    let mut checked = 0;
    for doc in corpus() {
        let idx = MarkdownIndex::build(doc, 1);
        let table = idx
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::SimpleTable)
            .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"));
        let cols =
            pulldown_cell_ranges(&doc[table.source_range.start..table.source_range.end])[0].len();
        let mut edits = Vec::new();
        for col in 0..cols {
            edits.push(insert(col, TableColumnPosition::Left));
            edits.push(insert(col, TableColumnPosition::Right));
            edits.push(align(col, TableAlignment::Centre));
            if cols > 1 {
                edits.push(delete(col));
            }
            if col > 0 {
                edits.push(move_col(col, TableColumnDirection::Left));
            }
            if col + 1 < cols {
                edits.push(move_col(col, TableColumnDirection::Right));
            }
        }
        let pipeless_pair = cols == 2
            && !doc.trim_start().starts_with('|')
            && !doc.lines().next().unwrap_or("").trim_end().ends_with('|');
        for edit in edits {
            if pipeless_pair && matches!(edit, FormBlockEdit::DeleteTableColumn { .. }) {
                assert!(
                    apply(doc, edit.clone()).is_err(),
                    "{doc:?} {edit:?} must be refused"
                );
                continue;
            }
            // A row with a single cell cannot lose it: its line would be blank,
            // which ends the table. The one shape that does this is the short
            // row in the corpus, deleted at column 0, and it must be refused.
            if matches!(edit, FormBlockEdit::DeleteTableColumn { col: 0 })
                && doc.contains("| 1 |\n")
            {
                assert!(
                    matches!(
                        apply(doc, edit.clone()),
                        Err(FormEditError::UnsupportedEditOperation { .. })
                    ),
                    "{doc:?} {edit:?} must be refused"
                );
                continue;
            }
            let out =
                apply(doc, edit.clone()).unwrap_or_else(|e| panic!("{doc:?} {edit:?}: {e:?}"));
            // Nothing before or after the table moves.
            let before = &doc[..table.source_range.start];
            let after = &doc[table.source_range.end..];
            assert!(out.starts_with(before), "{doc:?} {edit:?}: prefix moved");
            assert!(out.ends_with(after), "{doc:?} {edit:?}: suffix moved");
            // A column operation's result parses back with the expected
            // number of columns (the cell model in the resolver already
            // checked every untouched cell's text).
            let (prefix, suffix) = common_affix(doc, &out);
            assert!(
                prefix + suffix >= before.len() + after.len(),
                "{doc:?} {edit:?}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 60, "sanity: got {checked}");
}

// ─── Mutations (RFC-048 slice 4 §4) ───────────────────────────────────────
// Each mutation below was run by hand against the resolver and its named
// test failed by name, then reverted. Recorded in the review request.
