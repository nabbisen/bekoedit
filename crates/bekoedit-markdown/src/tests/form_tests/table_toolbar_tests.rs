// RFC-048 slice 2 §2.3: the inline toolbar acting on one table cell's
// selection. Headless, at the resolver level -- no WebView needed, per
// the handoff's §4 ("A WebView end-to-end check is not required in this
// slice"). Split out of `table_tests.rs` per the ELOC guideline.

use crate::block::BlockKind;
use crate::form::{FormBlockEdit, FormEditCommand, InlineFormat, resolve_form_edit};
use crate::index::MarkdownIndex;
use crate::patch::apply_patch;

fn toggle_in_cell(
    doc: &str,
    row: usize,
    col: usize,
    kind: InlineFormat,
    utf16_start: usize,
    utf16_len: usize,
) -> String {
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
        edit: FormBlockEdit::ToggleInlineInTableCell {
            row,
            col,
            kind,
            utf16_start,
            utf16_len,
            link_url: None,
        },
    };
    let patch = resolve_form_edit(doc, &idx, &cmd).unwrap();
    let mut out = doc.to_string();
    apply_patch(&mut out, 1, &patch).unwrap();
    out
}

#[test]
fn bold_applied_to_a_selection_in_one_cell_changes_only_that_cell() {
    let doc = "| Name | Score |\n|------|-------|\n| Alice | 42 |\n";
    // "Alice" is row 1, col 0; select all 5 UTF-16 code units of it.
    let out = toggle_in_cell(doc, 1, 0, InlineFormat::Bold, 0, 5);
    assert_eq!(
        out,
        "| Name | Score |\n|------|-------|\n| **Alice** | 42 |\n"
    );
}

#[test]
fn bold_toggled_again_on_the_same_selection_unwraps_it() {
    let doc = "| Name | Score |\n|------|-------|\n| **Alice** | 42 |\n";
    // The displayed (unescaped) cell text is "**Alice**", 9 UTF-16 units.
    let out = toggle_in_cell(doc, 1, 0, InlineFormat::Bold, 0, 9);
    assert_eq!(out, "| Name | Score |\n|------|-------|\n| Alice | 42 |\n");
}

#[test]
fn italic_on_a_partial_selection_wraps_only_that_part() {
    let doc = "| Name | Score |\n|------|-------|\n| Alice Smith | 42 |\n";
    // "Alice" only: UTF-16 offsets 0..5 of the cell's own text.
    let out = toggle_in_cell(doc, 1, 0, InlineFormat::Italic, 0, 5);
    assert_eq!(
        out,
        "| Name | Score |\n|------|-------|\n| _Alice_ Smith | 42 |\n"
    );
}

#[test]
fn code_applied_in_a_cell_with_an_escaped_pipe_changes_only_the_selection() {
    let doc = "| a | b |\n|---|---|\n| x \\| y | z |\n";
    // The displayed cell text is "x | y" (5 UTF-16 units); select "y" (one
    // unit, offset 4).
    let out = toggle_in_cell(doc, 1, 0, InlineFormat::Code, 4, 1);
    assert_eq!(out, "| a | b |\n|---|---|\n| x \\| `y` | z |\n");
}

#[test]
fn code_around_a_backtick_already_in_a_cell_uses_a_longer_fence() {
    // Confirms task 047 Part C's fix lands here too: the cell's wrap goes
    // through the same shared `toggled_text` as a paragraph's.
    let doc = "| a | b |\n|---|---|\n| x | a`b |\n";
    let out = toggle_in_cell(doc, 1, 1, InlineFormat::Code, 0, 3);
    assert_eq!(out, "| a | b |\n|---|---|\n| x | ``a`b`` |\n");
}

#[test]
fn bold_in_a_japanese_cell_changes_only_that_cell() {
    let doc = "| 名前 | 点数 |\n|------|------|\n| 太郎 | 90 |\n";
    let out = toggle_in_cell(doc, 1, 0, InlineFormat::Bold, 0, 2);
    assert_eq!(out, "| 名前 | 点数 |\n|------|------|\n| **太郎** | 90 |\n");
}
