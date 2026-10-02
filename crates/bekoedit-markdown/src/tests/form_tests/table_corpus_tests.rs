// RFC-027 simple table editing: the exhaustive corpus checks, split out of
// `table_tests.rs` per the ELOC guideline (see `form_tests.rs`). Shared
// helpers (`apply_table`, `projection`, `common_affix`, `assert_only_change_is`,
// `test_unescape_pipe`, `pulldown_cell_ranges`) live there, `pub(super)`.

use super::table_tests::{
    assert_only_change_is, common_affix, projection, pulldown_cell_ranges, test_unescape_pipe,
};
use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

/// Table sources covering: with/without outer pipes, padded/unpadded,
/// CRLF and LF, an escaped pipe, both empty-cell spellings, Japanese
/// text, and a cell each with a link, an image, an autolink, and an
/// entity (task 045 §4's required coverage, plus the review's
/// 2026-10-02 §4 addition).
fn corpus() -> Vec<&'static str> {
    vec![
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        "a|b\n-|-\n1|2\n",
        "|a|b|\n|-|-|\n|1|2|\n",
        "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n",
        "| a | b |\n|---|---|\n| x \\| y | 2 |\n",
        "| a | b |\n|---|---|\n|  | 2 |\n",
        "| a | b |\n|---|---|\n|| 2 |\n",
        "| 名前 | 年齢 |\n|------|------|\n| 太郎 | 20 |\n",
        "|   a   |   b   |\n|-------|-------|\n|   1   |   2   |\n",
        "| a | b |\n|---|---|\n| [t](u) | 2 |\n",
        "| a | b |\n|---|---|\n| ![a](s) | 2 |\n",
        "| a | b |\n|---|---|\n| <http://x> | 2 |\n",
        "| a | b |\n|---|---|\n| x &amp; y | 2 |\n",
    ]
}

#[test]
fn the_projection_and_the_edit_path_see_the_same_columns_for_every_corpus_table() {
    for doc in corpus() {
        let idx = MarkdownIndex::build(doc, 1);
        let table = idx
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::SimpleTable)
            .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"));
        let source = &doc[table.source_range.start..table.source_range.end];
        let ranges = pulldown_cell_ranges(source);
        let (headers, rows) = projection(doc);
        let mut projected = vec![headers];
        projected.extend(rows);
        assert_eq!(projected.len(), ranges.len(), "{doc:?}: row count");
        for (r, row_ranges) in ranges.iter().enumerate() {
            assert_eq!(
                projected[r].len(),
                row_ranges.len(),
                "{doc:?} row {r}: cell count"
            );
            for (c, range) in row_ranges.iter().enumerate() {
                let expected = test_unescape_pipe(&source[range.clone()]);
                assert_eq!(projected[r][c], expected, "{doc:?} row {r} col {c}");
            }
        }
    }
}

#[test]
fn every_cell_of_every_corpus_table_changes_only_that_cell() {
    let mut checked = 0;
    for doc in corpus() {
        let idx = MarkdownIndex::build(doc, 1);
        let table = idx
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::SimpleTable)
            .unwrap_or_else(|| panic!("{doc:?} must classify as a SimpleTable"));
        let source = &doc[table.source_range.start..table.source_range.end];
        let before_ranges = pulldown_cell_ranges(source);
        let row_count = before_ranges.len();
        for row in 0..row_count {
            let col_count = before_ranges[row].len();
            for col in 0..col_count {
                // A literal `|` in the typed text, on every iteration, so
                // this loop also exercises escaping it back into the
                // source, not only the already-escaped cells the corpus
                // happens to contain untouched.
                let new_text = format!("ED|TED-{row}-{col}");
                let cmd = FormEditCommand {
                    base_revision: 1,
                    block_id: table.block_id,
                    client_block_fingerprint: None,
                    edit: FormBlockEdit::ReplaceTableCell {
                        row,
                        col,
                        text: new_text.clone(),
                    },
                };
                let patch = resolve_form_edit(doc, &idx, &cmd)
                    .unwrap_or_else(|e| panic!("{doc:?} row {row} col {col}: {e:?}"));
                let mut out = doc.to_string();
                apply_patch(&mut out, 1, &patch).unwrap();

                // Only this one cell's bytes may differ, anywhere in the
                // document, not only inside the table.
                let escaped = new_text.replace('|', "\\|");
                assert_only_change_is(doc, &out, &escaped);

                // The result re-parses to the same shape; every other
                // cell's own raw source bytes are unchanged (not its
                // rendered text, which a link/image/autolink/entity cell
                // would not preserve), and the edited cell holds the new
                // escaped text, in both the resolver's own boundaries and
                // the projection shown for the new document.
                let after_idx = MarkdownIndex::build(&out, 2);
                let after_table = after_idx
                    .blocks
                    .iter()
                    .find(|b| b.kind == BlockKind::SimpleTable)
                    .unwrap_or_else(|| {
                        panic!("{doc:?} row {row} col {col}: table lost after edit")
                    });
                let after_source =
                    &out[after_table.source_range.start..after_table.source_range.end];
                let after_ranges = pulldown_cell_ranges(after_source);
                assert_eq!(after_ranges.len(), row_count, "{doc:?} row {row} col {col}");
                let (after_headers, after_rows) = projection(&out);
                let mut after_projected = vec![after_headers];
                after_projected.extend(after_rows);
                for (r, before_row) in before_ranges.iter().enumerate() {
                    assert_eq!(after_ranges[r].len(), before_row.len(), "{doc:?} row {r}");
                    for (c, before_range) in before_row.iter().enumerate() {
                        let expected_source = if r == row && c == col {
                            escaped.clone()
                        } else {
                            source[before_range.clone()].to_string()
                        };
                        assert_eq!(
                            &after_source[after_ranges[r][c].clone()],
                            expected_source.as_str(),
                            "{doc:?} editing row {row} col {col}, checking row {r} col {c}"
                        );
                        let expected_projection = if r == row && c == col {
                            new_text.clone()
                        } else {
                            test_unescape_pipe(&source[before_range.clone()])
                        };
                        assert_eq!(
                            after_projected[r][c], expected_projection,
                            "{doc:?} editing row {row} col {col}: projection at row {r} col {c}"
                        );
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 20,
        "sanity: the corpus should exercise at least 20 cells, got {checked}"
    );
}

#[test]
fn add_table_row_reparses_with_one_more_row_for_every_corpus_table() {
    for doc in corpus() {
        let idx = MarkdownIndex::build(doc, 1);
        let table = idx
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::SimpleTable)
            .unwrap();
        let before_source = &doc[table.source_range.start..table.source_range.end];
        let before_ranges = pulldown_cell_ranges(before_source);
        let cmd = FormEditCommand {
            base_revision: 1,
            block_id: table.block_id,
            client_block_fingerprint: None,
            edit: FormBlockEdit::AddTableRow,
        };
        let patch = resolve_form_edit(doc, &idx, &cmd).unwrap_or_else(|e| panic!("{doc:?}: {e:?}"));
        let mut out = doc.to_string();
        apply_patch(&mut out, 1, &patch).unwrap();

        let (prefix, suffix) = common_affix(doc, &out);
        assert_eq!(
            prefix + suffix,
            doc.len(),
            "{doc:?}: old bytes must be a prefix+suffix of the new document"
        );

        let after_idx = MarkdownIndex::build(&out, 2);
        let after_table = after_idx
            .blocks
            .iter()
            .find(|b| b.kind == BlockKind::SimpleTable)
            .unwrap();
        let after_source = &out[after_table.source_range.start..after_table.source_range.end];
        let after_ranges = pulldown_cell_ranges(after_source);
        assert_eq!(after_ranges.len(), before_ranges.len() + 1, "{doc:?}");
        let last = after_ranges.last().unwrap();
        assert!(
            last.iter().all(|r| after_source[r.clone()].is_empty()),
            "{doc:?}: the new row must be all empty cells"
        );
    }
}
