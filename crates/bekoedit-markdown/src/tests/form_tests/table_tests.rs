// RFC-027 simple table editing tests.

use crate::block::{BlockKind, EditablePolicy};
use crate::form::{
    FormBlockDisplay, FormBlockEdit, FormEditCommand, FormProjection, resolve_form_edit,
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
