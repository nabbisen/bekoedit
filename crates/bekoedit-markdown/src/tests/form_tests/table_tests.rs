// RFC-027 simple table editing tests.
//
// Task 045: a cell edit must change only that cell's own bytes. These
// tests assert exact output bytes, not just the projection, because the
// defect this task fixes (RFC-027's original `render_table` rewrote the
// whole table on every edit) was invisible to a projection-only check --
// the projection looked the same even when the separator's alignment,
// an untouched cell, or the table's own padding had silently changed.

use crate::block::{BlockKind, EditablePolicy};
use crate::form::{
    FormBlockDisplay, FormBlockEdit, FormEditCommand, FormEditError, FormProjection,
    resolve_form_edit,
};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn apply_table(doc: &str, edit: FormBlockEdit) -> String {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap();
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: table.block_id,
        client_block_fingerprint: None,
        edit,
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).unwrap();
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    out
}

fn try_replace_cell(
    doc: &str,
    row: usize,
    col: usize,
    text: &str,
) -> Result<String, FormEditError> {
    let idx = MarkdownIndex::build(doc, 1);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .ok_or(FormEditError::BlockNotFound)?;
    let cmd = FormEditCommand {
        base_revision: 1,
        block_id: table.block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplaceTableCell {
            row,
            col,
            text: text.to_string(),
        },
    };
    let patch = resolve_form_edit(doc, &idx, &cmd)?;
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    Ok(out)
}

fn projection(doc: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let idx = MarkdownIndex::build(doc, 1);
    let proj = FormProjection::build(doc, &idx);
    proj.blocks
        .iter()
        .find_map(|b| {
            if let FormBlockDisplay::Table {
                ref headers,
                ref rows,
                ..
            } = b.display
            {
                Some((headers.clone(), rows.clone()))
            } else {
                None
            }
        })
        .expect("table block in projection")
}

/// What `pulldown-cmark` itself parses a standalone table source as: each
/// row's cell texts, header first. An independent re-derivation from the
/// production oracle in `form::tables`, kept separate on purpose so this
/// test does not trust the same code path it is checking.
fn pulldown_cells(source: &str) -> Vec<Vec<String>> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Option<Vec<String>> = None;
    let mut cell: Option<String> = None;
    for (event, _) in Parser::new_ext(source, Options::ENABLE_TABLES).into_offset_iter() {
        match event {
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => row = Some(Vec::new()),
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                rows.push(row.take().expect("row was open"));
            }
            Event::Start(Tag::TableCell) => cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => row
                .as_mut()
                .expect("cell inside a row")
                .push(cell.take().expect("cell was open")),
            Event::Text(text) => {
                if let Some(c) = cell.as_mut() {
                    c.push_str(&text);
                }
            }
            _ => {}
        }
    }
    rows
}

/// Byte offsets `(prefix_len, suffix_len)` such that `old[..prefix_len]`
/// and `old[old.len()-suffix_len..]` are identical to the corresponding
/// ends of `new` -- the standard longest-common-affix diff, clamped to
/// UTF-8 boundaries (both strings only ever diverge at a boundary a
/// validated `ByteRange` produced, so clamping is defensive, not load
/// bearing).
fn common_affix(old: &str, new: &str) -> (usize, usize) {
    let (ob, nb) = (old.as_bytes(), new.as_bytes());
    let mut prefix = ob.iter().zip(nb).take_while(|(a, b)| a == b).count();
    while prefix > 0 && !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = ob.len().min(nb.len()) - prefix.min(ob.len().min(nb.len()));
    let mut suffix = ob[prefix..]
        .iter()
        .rev()
        .zip(nb[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(max_suffix);
    while suffix > 0 && !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    (prefix, suffix)
}

/// Asserts that every byte `new` differs from `old` in falls inside one
/// contiguous span, and that span's own text is exactly
/// `expected_replacement` -- task 045's core invariant: an edit changes
/// only the bytes the user changed, nothing else, anywhere in the
/// document.
fn assert_only_change_is(old: &str, new: &str, expected_replacement: &str) {
    let (prefix, suffix) = common_affix(old, new);
    let new_middle = &new[prefix..new.len() - suffix];
    assert_eq!(
        new_middle, expected_replacement,
        "expected the only change to be {expected_replacement:?}, but the \
         full documents were:\nold: {old:?}\nnew: {new:?}"
    );
}

#[test]
fn simple_table_is_form_editable_block() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let idx = MarkdownIndex::build(doc, 1);
    let t = idx.blocks.iter().find(|b| b.kind == BlockKind::SimpleTable);
    assert!(t.is_some());
    assert_eq!(t.unwrap().editable_policy, EditablePolicy::FormEditable);
}

#[test]
fn bold_table_stays_complex_island() {
    let doc = "| **Name** | Score |\n|----------|-------|\n| Alice | 42 |\n";
    let idx = MarkdownIndex::build(doc, 1);
    let t = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::ComplexTable);
    assert!(t.is_some());
}

#[test]
fn edit_header_cell() {
    let doc = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 0,
            col: 0,
            text: "Person".into(),
        },
    );
    let (headers, _) = projection(&out);
    assert_eq!(headers[0], "Person");
    assert_eq!(headers[1], "Age");
}

#[test]
fn edit_data_cell_preserves_structure() {
    let doc = "| Name | Age |\n|------|-----|\n| Alice | 30 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "Bob".into(),
        },
    );
    let (_, rows) = projection(&out);
    assert_eq!(rows[0][0], "Bob");
    assert_eq!(rows[0][1], "30");
}

#[test]
fn add_row_appends_empty_row() {
    let doc = "| x | y |\n|---|---|\n| a | b |\n";
    let out = apply_table(doc, FormBlockEdit::AddTableRow);
    let (_, rows) = projection(&out);
    assert_eq!(rows.len(), 2);
    assert!(rows[1].iter().all(|c| c.is_empty()));
}

#[test]
fn table_round_trip_is_source_preserving() {
    let doc = "before\n\n| x | y |\n|---|---|\n| a | b |\n\nafter\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "Z".into(),
        },
    );
    assert!(out.starts_with("before\n\n"));
    assert!(out.ends_with("\n\nafter\n"));
}

// --- Task 045 §1's five regression cases, each asserting exact bytes ---

#[test]
fn editing_one_cell_keeps_every_columns_alignment() {
    let doc = "| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "X".into(),
        },
    );
    assert_eq!(out, "| a | b | c |\n|:--|:-:|--:|\n| X | 2 | 3 |\n");
}

#[test]
fn an_escaped_pipe_in_an_untouched_cell_is_not_lost() {
    let doc = "| a | b |\n|---|---|\n| x \\| y | z |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "Q".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| x \\| y | Q |\n");
}

#[test]
fn a_typed_pipe_is_escaped_and_does_not_add_a_column() {
    let doc = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "p|q".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| p\\|q | 2 |\n");
    // Checked against pulldown-cmark directly, not `projection()`: the
    // Form Mode display's own `parse_simple_table` (in `form.rs`, not
    // touched by this task) still splits on every `|` without respecting
    // escapes, so it would misreport this row as 3 cells. See this
    // task's review request for that pre-existing, separate finding.
    let idx = MarkdownIndex::build(&out, 2);
    let table = idx
        .blocks
        .iter()
        .find(|b| b.kind == BlockKind::SimpleTable)
        .unwrap();
    let source = &out[table.source_range.start..table.source_range.end];
    let rows = pulldown_cells(source);
    assert_eq!(rows[1].len(), 2, "the row must still have exactly 2 cells");
    assert_eq!(rows[1][0], "p|q");
}

#[test]
fn an_unpadded_table_changes_only_the_edited_cell() {
    let doc = "|a|b|\n|-|-|\n|1|2|\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "Z".into(),
        },
    );
    assert_eq!(out, "|a|b|\n|-|-|\n|1|Z|\n");
}

#[test]
fn a_japanese_header_changes_only_the_edited_cell() {
    let doc = "| 名前 | 年齢 |\n|------|------|\n| Alice | 30 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 0,
            col: 0,
            text: "氏名".into(),
        },
    );
    assert_eq!(out, "| 氏名 | 年齢 |\n|------|------|\n| Alice | 30 |\n");
}

// --- The pulldown-cmark disagreement path ---

#[test]
fn a_cell_with_a_non_pipe_backslash_escape_is_refused_not_guessed_at() {
    // pulldown-cmark resolves `\*` to `*` inside a cell, same as anywhere
    // else inline text is parsed; this module's own splitter only ever
    // unescapes `\|` (task 045 §2.2's Projection rule), so the two
    // disagree on this cell's text, and the whole table must be refused
    // rather than silently corrupted.
    let doc = "| a | b |\n|---|---|\n| x | \\*y\\* |\n";
    let err = try_replace_cell(doc, 1, 0, "Q").unwrap_err();
    assert!(
        matches!(err, FormEditError::UnsupportedEditOperation { .. }),
        "expected a refusal, got {err:?}"
    );
}

// --- AddTableRow: a pure insertion ---

#[test]
fn add_table_row_is_a_prefix_and_suffix_preserving_insertion() {
    let doc = "before\n\n| x | y | z |\n|---|---|---|\n| a | b | c |\n\nafter\n";
    let out = apply_table(doc, FormBlockEdit::AddTableRow);
    let (prefix, suffix) = common_affix(doc, &out);
    assert_eq!(
        prefix + suffix,
        doc.len(),
        "the old document's bytes must split cleanly into a prefix and a \
         suffix around the insertion, with nothing removed or rewritten"
    );
    assert_eq!(&out[..prefix], &doc[..prefix]);
    assert_eq!(&out[out.len() - suffix..], &doc[doc.len() - suffix..]);
    let (_, rows) = projection(&out);
    assert_eq!(rows.len(), 2);
    assert!(rows[1].iter().all(|c| c.is_empty()));
}

// --- The exhaustive corpus: every cell of every table, edited, checked ---

/// Table sources covering: with/without outer pipes, padded/unpadded,
/// CRLF and LF, an escaped pipe, an already-empty cell, and Japanese
/// text (task 045 §4's required corpus coverage).
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
    ]
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
        let before_rows = pulldown_cells(source);
        let row_count = before_rows.len();
        for row in 0..row_count {
            let col_count = before_rows[row].len();
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

                // The result re-parses to the same shape, with the new
                // text in exactly this cell and every other cell
                // unchanged.
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
                let after_rows = pulldown_cells(after_source);
                assert_eq!(after_rows.len(), row_count, "{doc:?} row {row} col {col}");
                for (r, before_row) in before_rows.iter().enumerate() {
                    assert_eq!(after_rows[r].len(), before_row.len(), "{doc:?} row {r}");
                    for (c, before_cell) in before_row.iter().enumerate() {
                        let expected = if r == row && c == col {
                            &new_text
                        } else {
                            before_cell
                        };
                        assert_eq!(
                            &after_rows[r][c], expected,
                            "{doc:?} editing row {row} col {col}, checking row {r} col {c}"
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
        let before_rows = pulldown_cells(before_source);
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
        let after_rows = pulldown_cells(after_source);
        assert_eq!(after_rows.len(), before_rows.len() + 1, "{doc:?}");
        assert!(
            after_rows.last().unwrap().iter().all(|c| c.is_empty()),
            "{doc:?}: the new row must be all empty cells"
        );
    }
}
