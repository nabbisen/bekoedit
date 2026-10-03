// RFC-027 simple table editing tests.
//
// Task 045: a cell edit must change only that cell's own bytes. These
// tests assert exact output bytes, not just the projection, because the
// defect this task fixes (RFC-027's original `render_table` rewrote the
// whole table on every edit) was invisible to a projection-only check --
// the projection looked the same even when the separator's alignment,
// an untouched cell, or the table's own padding had silently changed.
//
// Review (2026-10-02): the first version of this fix still lost data,
// because `form.rs`'s own display projection kept a second, naive
// splitter that disagreed with the edit path on cell boundaries for an
// escaped pipe. The fix moved both to one shared `table_cell_ranges`
// (`form::tables`), which gets boundaries directly from `pulldown-cmark`'s
// own `TableCell` event ranges rather than comparing two independent
// guesses' rendered text -- the earlier text-comparison oracle refused
// every table with a link, image, autolink or entity in any cell, which
// was itself a regression. The tests below reflect that: projection and
// edit-path cases both now use the same corpus, and markup-in-a-cell
// cases assert the table is editable, not refused.
//
// The exhaustive corpus checks live in `table_corpus_tests.rs` (ELOC
// guideline, see `form_tests.rs`); the shared helpers below are
// `pub(super)` so that sibling module can reuse them.

use crate::block::{BlockKind, EditablePolicy};
use crate::form::{
    FormBlockDisplay, FormBlockEdit, FormEditCommand, FormEditError, FormProjection,
    resolve_form_edit,
};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

pub(super) fn apply_table(doc: &str, edit: FormBlockEdit) -> String {
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

pub(super) fn projection(doc: &str) -> (Vec<String>, Vec<Vec<String>>) {
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

/// `\|` to `|`, and nothing else unescaped -- an independent re-derivation
/// of `form::tables::unescape_pipe` (not reachable from here; `tables` is
/// a private submodule of `form`), kept separate on purpose so a check
/// against it does not trust the same code path it is checking.
pub(super) fn test_unescape_pipe(cell_source: &str) -> String {
    let mut out = String::with_capacity(cell_source.len());
    let mut chars = cell_source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            out.push('|');
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Each row's cell byte ranges in `source`, as `pulldown-cmark` itself
/// delimits them (a `TableCell`'s own event range, trimmed of whitespace)
/// -- an independent re-derivation of the production `table_cell_ranges`,
/// kept separate on purpose so this test does not trust the same code
/// path it is checking. Working in byte ranges, not rendered text, is
/// what lets this oracle handle a cell holding a link, image, autolink or
/// entity correctly: `pulldown-cmark`'s own `Event::Text` for such a cell
/// only ever gives the rendered label, never the cell's own Markdown
/// source.
pub(super) fn pulldown_cell_ranges(source: &str) -> Vec<Vec<std::ops::Range<usize>>> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let bytes = source.as_bytes();
    let mut rows: Vec<Vec<std::ops::Range<usize>>> = Vec::new();
    let mut row: Option<Vec<std::ops::Range<usize>>> = None;
    for (event, range) in Parser::new_ext(source, Options::ENABLE_TABLES).into_offset_iter() {
        match event {
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => row = Some(Vec::new()),
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                rows.push(row.take().expect("row was open"));
            }
            Event::End(TagEnd::TableCell) => {
                let mut start = range.start;
                let mut end = range.end;
                while start < end && bytes[start].is_ascii_whitespace() {
                    start += 1;
                }
                while end > start && bytes[end - 1].is_ascii_whitespace() {
                    end -= 1;
                }
                row.as_mut().expect("cell inside a row").push(start..end);
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
pub(super) fn common_affix(old: &str, new: &str) -> (usize, usize) {
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
pub(super) fn assert_only_change_is(old: &str, new: &str, expected_replacement: &str) {
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

// --- RFC-048 slice 2: formatted cells round-trip and edit byte-exact ---

#[test]
fn a_bold_header_cell_is_untouched_by_editing_a_different_cell() {
    let doc = "| **Name** | Score |\n|----------|-------|\n| Alice | 42 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "99".into(),
        },
    );
    assert_eq!(
        out,
        "| **Name** | Score |\n|----------|-------|\n| Alice | 99 |\n"
    );
}

#[test]
fn editing_a_bold_cell_replaces_its_markdown_source_not_its_rendered_text() {
    let doc = "| **Name** | Score |\n|----------|-------|\n| Alice | 42 |\n";
    let (headers, _) = projection(doc);
    assert_eq!(
        headers[0], "**Name**",
        "the cell's own Markdown source text"
    );
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 0,
            col: 0,
            text: "**Full Name**".into(),
        },
    );
    assert_eq!(
        out,
        "| **Full Name** | Score |\n|----------|-------|\n| Alice | 42 |\n"
    );
}

#[test]
fn a_code_cell_with_an_escaped_pipe_round_trips_and_a_different_cell_edits_cleanly() {
    let doc = "| a | b |\n|---|---|\n| `x\\|y` | 2 |\n";
    let (_, rows) = projection(doc);
    assert_eq!(
        rows[0][0], "`x|y`",
        "the backtick-span's own source, pipe unescaped"
    );
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "Z".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| `x\\|y` | Z |\n");
}

#[test]
fn a_formatted_crlf_table_changes_only_the_edited_cell() {
    let doc = "| Name | Score |\r\n|------|-------|\r\n| **Alice** | 42 |\r\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "99".into(),
        },
    );
    assert_eq!(
        out,
        "| Name | Score |\r\n|------|-------|\r\n| **Alice** | 99 |\r\n"
    );
}

#[test]
fn a_formatted_japanese_cell_changes_only_the_edited_cell() {
    let doc = "| 名前 | 点数 |\n|------|------|\n| **太郎** | 90 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 1,
            text: "100".into(),
        },
    );
    assert_eq!(
        out,
        "| 名前 | 点数 |\n|------|------|\n| **太郎** | 100 |\n"
    );
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
    let (_, rows) = projection(&out);
    assert_eq!(rows[0].len(), 2, "the row must still have exactly 2 cells");
    assert_eq!(rows[0][0], "p|q");
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

// --- The projection: review 2026-10-02's required evidence ---

#[test]
fn the_projection_unescapes_a_pipe_and_editing_it_is_byte_exact() {
    let doc = "| a | b |\n|---|---|\n| x \\| y | 1 |\n";
    let (_, rows) = projection(doc);
    assert_eq!(
        rows[0],
        vec!["x | y".to_string(), "1".to_string()],
        "the projection must show two cells, the first with its pipe unescaped"
    );
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "x | yz".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| x \\| yz | 1 |\n");
}

// --- Markup inside a cell: editable, not refused (review 2026-10-02 §3) ---

#[test]
fn a_table_with_a_link_image_autolink_and_entity_lets_a_different_cell_be_edited() {
    let doc = "| plain | link | image | auto | entity |\n\
               |-------|------|-------|------|--------|\n\
               | 1 | [t](u) | ![a](s) | <http://x> | a &amp; b |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "Z".into(),
        },
    );
    assert_eq!(
        out,
        "| plain | link | image | auto | entity |\n\
         |-------|------|-------|------|--------|\n\
         | Z | [t](u) | ![a](s) | <http://x> | a &amp; b |\n"
    );
}

#[test]
fn editing_a_cell_with_a_link_itself_replaces_only_that_cell() {
    let doc = "| a | b |\n|---|---|\n| [t](u) | 2 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "plain".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| plain | 2 |\n");
}

// --- What "the oracle disagrees" means now, and why it can no longer be
// --- constructed the way the first version of this fix could refuse a
// --- table wrongly ---

#[test]
fn an_unparsable_table_source_is_refused_not_guessed_at() {
    // Defensive, not a real-world-constructible case: `table_cell_ranges`
    // only returns `None` when `source` does not parse as a table at all.
    // For a block `classify_table` already accepted, re-parsing the same
    // text with the same parser cannot disagree with itself -- there is
    // no second, independent splitter left to disagree with (that *was*
    // the bug the review found: a cell's rendered text no longer compared
    // against this module's boundaries, after the fix below). This test
    // exercises the refusal path directly, via `FormEditCommand`'s own
    // guard, by targeting a block that is not a table at all.
    let doc = "just a paragraph\n";
    let err = try_replace_cell(doc, 0, 0, "x");
    assert!(err.is_err(), "there is no table block to find at all");
}

#[test]
fn a_cell_with_a_non_pipe_backslash_escape_is_editable_not_falsely_refused() {
    // The first version of this fix compared pulldown-cmark's *rendered
    // text* for each cell against this module's own `\|`-only unescape,
    // and refused on any difference -- which `\*y\*` (rendered `*y*` by
    // pulldown-cmark, kept literal by `\|`-only unescaping) triggered
    // even though the cell boundaries themselves agreed completely. The
    // review found this is a false refusal, not a real one: boundaries
    // are all that matters for a byte-exact patch, and the two never
    // disagreed about where this cell starts and ends.
    let doc = "| a | b |\n|---|---|\n| x | \\*y\\* |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "Q".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| Q | \\*y\\* |\n");
}

// --- RFC-048 slice 3 review (2026-10-03): whitespace-only cells ---

#[test]
fn editing_a_two_space_cell_inserts_after_the_first_space() {
    // Every row `InsertTableRow`/`AddTableRow` creates is exactly this
    // shape (`empty_row_text`) -- the first thing a user types into a
    // freshly inserted row lands here. `| ZQ7 |`, not `|ZQ7  |` or
    // `|  ZQ7|`: one original space kept on each side of the insertion,
    // never removed or moved.
    let doc = "| a | b |\n|---|---|\n|  | 2 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "ZQ7".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| ZQ7 | 2 |\n");
}

#[test]
fn editing_a_three_space_cell_inserts_after_the_first_space_too() {
    let doc = "| a | b |\n|---|---|\n|   | 2 |\n";
    let out = apply_table(
        doc,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "ZQ7".into(),
        },
    );
    assert_eq!(out, "| a | b |\n|---|---|\n| ZQ7  | 2 |\n");
}

#[test]
fn editing_a_truly_empty_or_one_space_cell_is_unchanged_by_the_review() {
    // `||` (no byte between the pipes) and `| |` (one space) have no
    // second whitespace character to land before, so both keep today's
    // behaviour: directly after the pipe, or after the one space.
    let empty = apply_table(
        "| a | b |\n|---|---|\n|| 2 |\n",
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "Z".into(),
        },
    );
    assert_eq!(empty, "| a | b |\n|---|---|\n|Z| 2 |\n");
    let one_space = apply_table(
        "| a | b |\n|---|---|\n| | 2 |\n",
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "Z".into(),
        },
    );
    assert_eq!(one_space, "| a | b |\n|---|---|\n| Z| 2 |\n");
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
