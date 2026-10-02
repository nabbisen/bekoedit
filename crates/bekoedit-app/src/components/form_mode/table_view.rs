//! Simple GFM table grid (RFC-027, extended by RFC-048 slice 2).
//!
//! Its own component, not inlined into `block_view.rs`'s match, so the
//! focused-cell signal below is only ever created while a table block is
//! actually mounted -- Dioxus hooks must be called in the same order on
//! every render of a component, so a signal cannot live inside one match
//! arm of a component that renders every block kind through one big match.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, fingerprint::BlockId};

use super::dispatch;
use super::inline_toolbar::InlineToolbar;
use crate::components::icons::AddIcon;
use crate::i18n::{Lang, tr};

#[component]
pub fn TableView(
    field_id: String,
    block_id: BlockId,
    revision: u64,
    lang: Lang,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_count: usize,
) -> Element {
    let state = use_context::<Signal<AppState>>();
    // One toolbar for the whole table, acting on whichever cell last took
    // focus (RFC-048 slice 2 §2.3), not one toolbar per cell.
    let mut focused = use_signal::<Option<(usize, usize)>>(|| None);

    rsx! {
        div { class: "table-block",
            if let Some((row, col)) = *focused.read() {
                InlineToolbar {
                    field_id: format!("{field_id}-{row}-{col}"),
                    block_id,
                    revision,
                    lang,
                    cell: Some((row, col)),
                }
            }
            table {
                thead {
                    tr {
                        for (ci, header) in headers.iter().enumerate() {
                            th {
                                input {
                                    id: "{field_id}-0-{ci}",
                                    r#type: "text",
                                    class: "table-cell-input",
                                    value: "{header}",
                                    aria_label: "{header}",
                                    onfocus: move |_| focused.set(Some((0, ci))),
                                    onchange: move |evt: Event<FormData>| dispatch(
                                        state,
                                        revision,
                                        block_id,
                                        FormBlockEdit::ReplaceTableCell { row: 0, col: ci, text: evt.value() },
                                    ),
                                }
                            }
                        }
                    }
                }
                tbody {
                    for (ri, row_cells) in rows.iter().enumerate() {
                        tr {
                            for (ci, cell) in row_cells.iter().enumerate() {
                                td {
                                    input {
                                        id: "{field_id}-{ri + 1}-{ci}",
                                        r#type: "text",
                                        class: "table-cell-input",
                                        value: "{cell}",
                                        // Column header plus row number
                                        // (RFC-048 §5.4), e.g. "Name, row 2".
                                        aria_label: {
                                            let header = headers.get(ci).map(String::as_str).unwrap_or("");
                                            format!("{header}, row {}", ri + 1)
                                        },
                                        onfocus: move |_| focused.set(Some((ri + 1, ci))),
                                        onchange: {
                                            let row_num = ri + 1;
                                            move |evt: Event<FormData>| dispatch(
                                                state,
                                                revision,
                                                block_id,
                                                FormBlockEdit::ReplaceTableCell {
                                                    row: row_num,
                                                    col: ci,
                                                    text: evt.value(),
                                                },
                                            )
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
            button {
                class: "table-add-row",
                onclick: move |_| dispatch(state, revision, block_id, FormBlockEdit::AddTableRow),
                AddIcon {}
                {tr(lang, "table.add_row")}
            }
            // Warn if col_count == 0 (degenerate table).
            if col_count == 0 {
                p { class: "muted", {tr(lang, "table.empty")} }
            }
        }
    }
}
