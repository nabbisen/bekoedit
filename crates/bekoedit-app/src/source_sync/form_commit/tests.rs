use super::*;
use bekoedit_markdown::MarkdownIndex;

fn projection_for(doc: &str) -> FormProjection {
    let index = MarkdownIndex::build(doc, 1);
    FormProjection::build(doc, &index)
}

fn paragraph_id(projection: &FormProjection) -> String {
    let block = projection
        .blocks
        .iter()
        .find(|b| matches!(b.display, FormBlockDisplay::Paragraph { .. }))
        .expect("a paragraph block");
    format!(
        "fb-{}-{}",
        block.block_id.ordinal, block.block_id.fingerprint.content_hash
    )
}

fn table_id(projection: &FormProjection) -> String {
    let block = projection
        .blocks
        .iter()
        .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
        .expect("a table block");
    format!(
        "fb-{}-{}",
        block.block_id.ordinal, block.block_id.fingerprint.content_hash
    )
}

// ---------------------------------------------------------------- parse_field_id

#[test]
fn parse_field_id_reads_a_plain_blocks_ordinal_and_hash() {
    assert_eq!(
        parse_field_id("fb-3-1234567890"),
        Some(ParsedFieldId {
            ordinal: 3,
            content_hash: 1234567890,
            cell: None
        })
    );
}

#[test]
fn parse_field_id_reads_a_table_cells_row_and_col_too() {
    assert_eq!(
        parse_field_id("fb-3-1234567890-2-1"),
        Some(ParsedFieldId {
            ordinal: 3,
            content_hash: 1234567890,
            cell: Some((2, 1))
        })
    );
}

#[test]
fn parse_field_id_rejects_anything_not_starting_with_fb() {
    assert_eq!(parse_field_id("other-3-1234567890"), None);
    assert_eq!(parse_field_id(""), None);
}

#[test]
fn parse_field_id_rejects_an_unparseable_number() {
    assert_eq!(parse_field_id("fb-x-1234567890"), None);
    assert_eq!(parse_field_id("fb-3-1234567890-y-1"), None);
}

#[test]
fn parse_field_id_rejects_the_wrong_number_of_parts() {
    assert_eq!(parse_field_id("fb-3"), None);
    assert_eq!(parse_field_id("fb-3-1234567890-2"), None);
}

// ------------------------------------------------------------------ current_text

#[test]
fn current_text_reads_a_paragraphs_own_text() {
    let projection = projection_for("A paragraph.\n");
    let display = &projection.blocks[0].display;
    assert_eq!(current_text(display, None), Some("A paragraph."));
}

#[test]
fn current_text_reads_a_table_header_at_row_zero() {
    let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let display = &projection
        .blocks
        .iter()
        .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
        .unwrap()
        .display;
    assert_eq!(current_text(display, Some((0, 1))), Some("b"));
}

#[test]
fn current_text_reads_a_table_data_cell_at_row_plus_one() {
    let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let display = &projection
        .blocks
        .iter()
        .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
        .unwrap()
        .display;
    assert_eq!(current_text(display, Some((1, 0))), Some("1"));
}

#[test]
fn current_text_is_none_for_a_cell_address_that_does_not_exist() {
    let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let display = &projection
        .blocks
        .iter()
        .find(|b| matches!(b.display, FormBlockDisplay::Table { .. }))
        .unwrap()
        .display;
    assert_eq!(current_text(display, Some((9, 9))), None);
}

#[test]
fn current_text_is_none_when_display_and_cell_disagree() {
    let projection = projection_for("A paragraph.\n");
    let display = &projection.blocks[0].display;
    // A cell address against a paragraph: not a table.
    assert_eq!(current_text(display, Some((0, 0))), None);
}

// ------------------------------------------------------------ resolve_pending_commit

#[test]
fn an_unchanged_value_resolves_to_no_commit() {
    let projection = projection_for("A paragraph.\n");
    let id = paragraph_id(&projection);
    let pending = PendingField {
        id,
        value: "A paragraph.".to_string(),
        composing: false,
    };
    assert_eq!(
        resolve_pending_commit(&pending, &projection),
        Err(NoCommitReason::ValueUnchanged)
    );
}

#[test]
fn a_changed_paragraph_resolves_to_a_replace_plain_text_commit() {
    let projection = projection_for("A paragraph.\n");
    let id = paragraph_id(&projection);
    let block_id = projection.blocks[0].block_id;
    let pending = PendingField {
        id,
        value: "A paragraph, changed.".to_string(),
        composing: false,
    };
    let cmd = resolve_pending_commit(&pending, &projection).expect("a commit");
    assert_eq!(cmd.base_revision, projection.document_revision);
    assert_eq!(cmd.block_id, block_id);
    assert_eq!(cmd.client_block_fingerprint, Some(block_id.fingerprint));
    assert_eq!(
        cmd.edit,
        FormBlockEdit::ReplacePlainText {
            text: "A paragraph, changed.".to_string()
        }
    );
}

#[test]
fn a_changed_table_cell_resolves_to_a_replace_table_cell_commit() {
    let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let id = format!("{}-1-0", table_id(&projection));
    let pending = PendingField {
        id,
        value: "99".to_string(),
        composing: false,
    };
    let cmd = resolve_pending_commit(&pending, &projection).expect("a commit");
    assert_eq!(
        cmd.edit,
        FormBlockEdit::ReplaceTableCell {
            row: 1,
            col: 0,
            text: "99".to_string()
        }
    );
}

#[test]
fn an_id_naming_no_block_in_the_projection_resolves_to_no_commit() {
    let projection = projection_for("A paragraph.\n");
    let pending = PendingField {
        id: "fb-99-999999999".to_string(),
        value: "anything".to_string(),
        composing: false,
    };
    assert_eq!(
        resolve_pending_commit(&pending, &projection),
        Err(NoCommitReason::NoMatchingBlock)
    );
}

#[test]
fn a_malformed_id_resolves_to_no_commit() {
    let projection = projection_for("A paragraph.\n");
    let pending = PendingField {
        id: "not-a-field-id".to_string(),
        value: "anything".to_string(),
        composing: false,
    };
    assert_eq!(
        resolve_pending_commit(&pending, &projection),
        Err(NoCommitReason::IdDidNotParse)
    );
}

#[test]
fn a_cell_address_the_table_does_not_have_resolves_to_kinds_did_not_match() {
    let projection = projection_for("| a | b |\n|---|---|\n| 1 | 2 |\n");
    let id = format!("{}-9-9", table_id(&projection));
    let pending = PendingField {
        id,
        value: "anything".to_string(),
        composing: false,
    };
    assert_eq!(
        resolve_pending_commit(&pending, &projection),
        Err(NoCommitReason::KindsDidNotMatch)
    );
}

// ---------------------------------------------------------- NoCommitReason

#[test]
fn each_reason_has_its_own_trace_label() {
    assert_eq!(
        NoCommitReason::IdDidNotParse.trace_label(),
        "the id did not parse"
    );
    assert_eq!(
        NoCommitReason::NoMatchingBlock.trace_label(),
        "no block matched"
    );
    assert_eq!(
        NoCommitReason::KindsDidNotMatch.trace_label(),
        "the kinds did not match"
    );
    assert_eq!(
        NoCommitReason::ValueUnchanged.trace_label(),
        "the value was unchanged"
    );
}
