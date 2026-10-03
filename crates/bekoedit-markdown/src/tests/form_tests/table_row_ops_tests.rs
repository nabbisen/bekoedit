// RFC-048 handoff, slice 3: row operations (insert, delete, move). Split
// out as its own module, next to `table_corpus_tests.rs`, per the ELOC
// guideline (see `form_tests.rs`). Shared helpers (`projection`,
// `common_affix`, `pulldown_cell_ranges`) live in `table_tests.rs`,
// `pub(super)`.

use super::table_tests::{common_affix, projection, pulldown_cell_ranges};
use crate::block::BlockKind;
use crate::form::{
    FormBlockEdit, FormEditCommand, FormEditError, TableRowDirection, TableRowPosition,
    resolve_form_edit,
};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

/// Three-data-row tables covering: with/without outer pipes,
/// padded/unpadded, CRLF and LF, the file's own last line with no
/// trailing newline, a table followed by a paragraph, Japanese text, an
/// escaped pipe, and a formatted cell (handoff §4). Every row's cells
/// hold distinct single-character values (`1`/`2`, `3`/`4`, `5`/`6`) so a
/// byte-level diff of a swap can never be ambiguous about which row
/// moved where.
fn corpus() -> Vec<&'static str> {
    vec![
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n",
        "a|b\n-|-\n1|2\n3|4\n5|6\n",
        "|a|b|\n|-|-|\n|1|2|\n|3|4|\n|5|6|\n",
        "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n| 3 | 4 |\r\n| 5 | 6 |\r\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |",
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n\nafter\n",
        "| 名前 | 年齢 |\n|------|------|\n| 太郎 | 20 |\n| 次郎 | 22 |\n| 三郎 | 24 |\n",
        "| a | b |\n|---|---|\n| x \\| y | 2 |\n| 3 | 4 |\n| 5 | 6 |\n",
        "| a | b |\n|---|---|\n| **bold** | 2 |\n| 3 | 4 |\n| 5 | 6 |\n",
    ]
}

fn table_block(doc: &str) -> (MarkdownIndex, crate::block::BlockNode) {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"))
        .clone();
    (idx, table)
}

fn apply(doc: &str, edit: FormBlockEdit) -> Result<String, FormEditError> {
    let (idx, table) = table_block(doc);
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

fn row_texts(doc: &str, row: usize) -> Vec<String> {
    let (headers, rows) = projection(doc);
    if row == 0 {
        headers
    } else {
        rows[row - 1].clone()
    }
}

// ─── Insert: exact bytes on canonical fixtures ─────────────────────────────

#[test]
fn insert_below_the_first_data_row_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 1,
            position: TableRowPosition::Below,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n|  |  |\n| 3 | 4 |\n");
}

#[test]
fn insert_above_a_data_row_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 2,
            position: TableRowPosition::Above,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n|  |  |\n| 3 | 4 |\n");
}

#[test]
fn insert_below_the_last_row_matches_add_table_row_with_no_trailing_newline() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 2,
            position: TableRowPosition::Below,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n|  |  |");
}

#[test]
fn insert_below_the_last_row_before_a_following_paragraph() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n\nafter\n";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 1,
            position: TableRowPosition::Below,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n|  |  |\n\nafter\n");
}

#[test]
fn insert_above_the_header_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let err = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 0,
            position: TableRowPosition::Above,
        },
    );
    assert!(matches!(
        err,
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

#[test]
fn insert_below_the_header_is_allowed() {
    // Inserting between the header and the first data row -- the exact
    // shape the end-to-end scenario drives through the real UI.
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 0,
            position: TableRowPosition::Below,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n|  |  |\n| 1 | 2 |\n");
}

#[test]
fn insert_at_an_out_of_range_row_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let err = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 5,
            position: TableRowPosition::Below,
        },
    );
    assert!(matches!(err, Err(FormEditError::ItemNotFound { .. })));
}

#[test]
fn a_new_row_never_has_the_short_row_shape_even_next_to_one() {
    // Task 045's re-review §2 accepted that editing a *missing* cell in a
    // short row (`| 1 |` in a two-column table) writes the minimal
    // `| 1 |Z`, not a padded `| 1 | Z |` -- out of scope to change here.
    // A freshly inserted row must still always be a full, padded row
    // regardless of what its neighbour looks like.
    let doc = "| a | b |\n|---|---|\n| 1 |\n| 3 | 4 |\n";
    let out = apply(
        doc,
        FormBlockEdit::InsertTableRow {
            at: 1,
            position: TableRowPosition::Below,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 |\n|  |  |\n| 3 | 4 |\n");
    // The short row itself is untouched.
    assert!(out.contains("| 1 |\n"));
}

#[test]
fn insert_over_the_corpus_is_a_prefix_and_suffix_preserving_insertion() {
    let mut checked = 0;
    for doc in corpus() {
        let (_, table) = table_block(doc);
        let before_rows =
            pulldown_cell_ranges(&doc[table.source_range.start..table.source_range.end]);
        let row_count = before_rows.len();
        for at in 1..row_count {
            for position in [TableRowPosition::Above, TableRowPosition::Below] {
                let out = apply(doc, FormBlockEdit::InsertTableRow { at, position })
                    .unwrap_or_else(|e| panic!("{doc:?} at={at} position={position:?}: {e:?}"));
                let (prefix, suffix) = common_affix(doc, &out);
                assert_eq!(
                    prefix + suffix,
                    doc.len(),
                    "{doc:?} at={at} position={position:?}: insertion must preserve every old byte"
                );
                let (_, idx_table) = table_block(&out);
                let after_rows = pulldown_cell_ranges(
                    &out[idx_table.source_range.start..idx_table.source_range.end],
                );
                assert_eq!(
                    after_rows.len(),
                    row_count + 1,
                    "{doc:?} at={at} position={position:?}"
                );
                let new_index = match position {
                    TableRowPosition::Above => at,
                    TableRowPosition::Below => at + 1,
                };
                let new_row = &after_rows[new_index];
                assert!(
                    new_row.iter().all(|r| out[r.clone()].trim().is_empty()),
                    "{doc:?} at={at} position={position:?}: the new row must be all empty cells"
                );
                // Every other row's own text is unchanged, at its shifted index.
                for (old_idx, old_row) in before_rows.iter().enumerate().take(row_count) {
                    let shifted = if old_idx >= new_index {
                        old_idx + 1
                    } else {
                        old_idx
                    };
                    for (col, old_range) in old_row.iter().enumerate() {
                        let before_source = &doc[table.source_range.start..table.source_range.end];
                        let expected = &before_source[old_range.clone()];
                        let after_source =
                            &out[idx_table.source_range.start..idx_table.source_range.end];
                        let actual = &after_source[after_rows[shifted][col].clone()];
                        assert_eq!(
                            actual, expected,
                            "{doc:?} at={at} position={position:?}: row {old_idx} col {col}"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "sanity: got {checked}");
}

// ─── Delete: exact bytes on canonical fixtures ─────────────────────────────

#[test]
fn delete_a_middle_row_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n";
    let out = apply(doc, FormBlockEdit::DeleteTableRow { row: 2 }).unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n| 5 | 6 |\n");
}

#[test]
fn delete_the_last_row_with_a_trailing_newline_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    let out = apply(doc, FormBlockEdit::DeleteTableRow { row: 2 }).unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |\n");
}

#[test]
fn delete_the_last_row_with_no_trailing_newline_removes_the_preceding_line_ending() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let out = apply(doc, FormBlockEdit::DeleteTableRow { row: 2 }).unwrap();
    // The new last line ("| 1 | 2 |") must not gain a trailing newline it
    // never had -- that would be an extra byte change outside "that
    // row's own line", and the file's own no-final-newline property
    // would flip for no reason.
    assert_eq!(out, "| a | b |\n|---|---|\n| 1 | 2 |");
    assert!(!out.ends_with('\n'));
}

#[test]
fn delete_the_only_data_row_leaves_a_header_only_table() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let out = apply(doc, FormBlockEdit::DeleteTableRow { row: 1 }).unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n");
    let (headers, rows) = projection(&out);
    assert_eq!(headers, vec!["a", "b"]);
    assert!(rows.is_empty());
}

#[test]
fn delete_the_header_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let err = apply(doc, FormBlockEdit::DeleteTableRow { row: 0 });
    assert!(matches!(
        err,
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

#[test]
fn delete_over_the_corpus_removes_only_that_row() {
    let mut checked = 0;
    for doc in corpus() {
        let (_, table) = table_block(doc);
        let before_source = &doc[table.source_range.start..table.source_range.end];
        let before_rows = pulldown_cell_ranges(before_source);
        let row_count = before_rows.len();
        for row in 1..row_count {
            let before_texts: Vec<Vec<String>> =
                (0..row_count).map(|r| row_texts(doc, r)).collect();
            let out = apply(doc, FormBlockEdit::DeleteTableRow { row })
                .unwrap_or_else(|e| panic!("{doc:?} row={row}: {e:?}"));
            let (_, idx_table) = table_block(&out);
            let after_source = &out[idx_table.source_range.start..idx_table.source_range.end];
            let after_rows = pulldown_cell_ranges(after_source);
            assert_eq!(after_rows.len(), row_count - 1, "{doc:?} row={row}");
            for (old_idx, before_text) in before_texts.iter().enumerate().take(row_count) {
                if old_idx == row {
                    continue;
                }
                let shifted = if old_idx > row { old_idx - 1 } else { old_idx };
                assert_eq!(
                    row_texts(&out, shifted),
                    *before_text,
                    "{doc:?} row={row}: checking old row {old_idx}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 20, "sanity: got {checked}");
}

// ─── Move: exact bytes on canonical fixtures ───────────────────────────────

#[test]
fn move_down_swaps_with_the_next_row_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n";
    let out = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 1,
            direction: TableRowDirection::Down,
        },
    )
    .unwrap();
    assert_eq!(
        out,
        "| a | b |\n|---|---|\n| 3 | 4 |\n| 1 | 2 |\n| 5 | 6 |\n"
    );
}

#[test]
fn move_up_swaps_with_the_previous_row_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n| 5 | 6 |\n";
    let out = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 3,
            direction: TableRowDirection::Up,
        },
    )
    .unwrap();
    assert_eq!(
        out,
        "| a | b |\n|---|---|\n| 1 | 2 |\n| 5 | 6 |\n| 3 | 4 |\n"
    );
}

#[test]
fn move_preserves_crlf_line_endings_in_their_original_positions() {
    let doc = "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n| 3 | 4 |\r\n| 5 | 6 |\r\n";
    let out = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 1,
            direction: TableRowDirection::Down,
        },
    )
    .unwrap();
    assert_eq!(
        out,
        "| a | b |\r\n|---|---|\r\n| 3 | 4 |\r\n| 1 | 2 |\r\n| 5 | 6 |\r\n"
    );
}

#[test]
fn move_preserves_each_line_endings_own_style_in_a_mixed_file() {
    // Row 1 ends CRLF, row 2 ends LF, row 3 ends CRLF -- a file with
    // genuinely mixed line endings, where "the line endings stay where
    // they were" and "the line endings swap along with the rows" would
    // produce different, both byte-exact, outputs. Only the former is
    // correct: the LE *between* the two swapped lines (CRLF, originally
    // between row 1 and row 2) stays between them; the LE *after* the
    // pair (LF, originally after row 2) stays after them, untouched.
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\r\n| 3 | 4 |\n| 5 | 6 |\r\n";
    let out = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 1,
            direction: TableRowDirection::Down,
        },
    )
    .unwrap();
    assert_eq!(
        out,
        "| a | b |\n|---|---|\n| 3 | 4 |\r\n| 1 | 2 |\n| 5 | 6 |\r\n"
    );
}

#[test]
fn move_the_last_row_up_with_no_trailing_newline_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let out = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 2,
            direction: TableRowDirection::Up,
        },
    )
    .unwrap();
    assert_eq!(out, "| a | b |\n|---|---|\n| 3 | 4 |\n| 1 | 2 |");
    assert!(!out.ends_with('\n'));
}

#[test]
fn move_up_from_the_first_data_row_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    let err = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 1,
            direction: TableRowDirection::Up,
        },
    );
    assert!(matches!(
        err,
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

#[test]
fn move_down_from_the_last_row_is_refused() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    let err = apply(
        doc,
        FormBlockEdit::MoveTableRow {
            row: 2,
            direction: TableRowDirection::Down,
        },
    );
    assert!(matches!(
        err,
        Err(FormEditError::UnsupportedEditOperation { .. })
    ));
}

#[test]
fn moving_the_header_is_refused_in_either_direction() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
    for direction in [TableRowDirection::Up, TableRowDirection::Down] {
        let err = apply(doc, FormBlockEdit::MoveTableRow { row: 0, direction });
        assert!(matches!(
            err,
            Err(FormEditError::UnsupportedEditOperation { .. })
        ));
    }
}

#[test]
fn move_over_the_corpus_swaps_only_the_two_rows() {
    let mut checked = 0;
    for doc in corpus() {
        let (_, table) = table_block(doc);
        let row_count =
            pulldown_cell_ranges(&doc[table.source_range.start..table.source_range.end]).len();
        for row in 1..row_count {
            for (direction, other) in [
                (TableRowDirection::Down, row + 1),
                (TableRowDirection::Up, row.wrapping_sub(1)),
            ] {
                if direction == TableRowDirection::Down && row + 1 >= row_count {
                    continue;
                }
                if direction == TableRowDirection::Up && row < 2 {
                    continue;
                }
                let before_texts: Vec<Vec<String>> =
                    (0..row_count).map(|r| row_texts(doc, r)).collect();
                let out = apply(doc, FormBlockEdit::MoveTableRow { row, direction })
                    .unwrap_or_else(|e| panic!("{doc:?} row={row} direction={direction:?}: {e:?}"));
                assert_eq!(
                    out.len(),
                    doc.len(),
                    "{doc:?} row={row} direction={direction:?}: a swap changes no byte count"
                );
                assert_eq!(
                    row_texts(&out, row),
                    before_texts[other],
                    "{doc:?} row={row} direction={direction:?}: row {row} now holds row {other}"
                );
                assert_eq!(
                    row_texts(&out, other),
                    before_texts[row],
                    "{doc:?} row={row} direction={direction:?}: row {other} now holds row {row}"
                );
                for (idx, before_text) in before_texts.iter().enumerate().take(row_count) {
                    if idx != row && idx != other {
                        assert_eq!(
                            row_texts(&out, idx),
                            *before_text,
                            "{doc:?} row={row} direction={direction:?}: row {idx} untouched"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 15, "sanity: got {checked}");
}

// ─── Refusal: not a table at all ───────────────────────────────────────────

#[test]
fn row_operations_on_a_non_table_block_are_refused() {
    let doc = "just a paragraph\n";
    let idx = MarkdownIndex::build(doc, 1);
    let block = &idx.blocks[0];
    assert_ne!(block.kind, BlockKind::SimpleTable);
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: block.block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::DeleteTableRow { row: 1 },
    };
    assert!(resolve_form_edit(doc, &idx, &cmd).is_err());
}
