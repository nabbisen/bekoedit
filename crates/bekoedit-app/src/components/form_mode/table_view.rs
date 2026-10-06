//! Simple GFM table grid (RFC-027, extended by RFC-048 slice 2).
//!
//! Its own component, not inlined into `block_view.rs`'s match, so the
//! focused-cell signal below is only ever created while a table block is
//! actually mounted -- Dioxus hooks must be called in the same order on
//! every render of a component, so a signal cannot live inside one match
//! arm of a component that renders every block kind through one big match.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, TableAlignment, fingerprint::BlockId};

use super::dispatch;
use super::inline_toolbar::InlineToolbar;
use super::table_column_menu::{ColumnActionsMenu, alignment_at, text_align_at};
use super::table_keys::cell_keydown;
use super::table_row_menu::RowActionsMenu;
use crate::components::icons::AddIcon;
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus::FocusMove;
use bekoedit_markdown::cell_plain_text;

#[component]
pub fn TableView(
    field_id: String,
    block_id: BlockId,
    revision: u64,
    lang: Lang,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    col_count: usize,
    alignments: Vec<TableAlignment>,
) -> Element {
    let state = use_context::<Signal<AppState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    // One toolbar for the whole table, acting on whichever cell last took
    // focus (RFC-048 slice 2 §2.3), not one toolbar per cell.
    let mut focused = use_signal::<Option<(usize, usize)>>(|| None);
    // RFC-048 slice 3 §2.2: one row-actions menu open at a time, by its
    // absolute row index (1-based; 0 is the header, which has no menu).
    let open_row_menu = use_signal::<Option<usize>>(|| None);
    // Task 017's own entry mechanism, scoped to this table's one menu
    // instead of a fixed `MENU_*` id: a closed menu opened by keyboard
    // records which item to focus, then the menu container's own
    // `onmounted` -- not a frame count or a poll -- moves focus once it
    // actually exists.
    let row_menu_entry = use_signal::<Option<FocusMove>>(|| None);
    // RFC-048 slice 4 §2.2: the same for one column header's menu.
    let open_col_menu = use_signal::<Option<usize>>(|| None);
    let col_menu_entry = use_signal::<Option<FocusMove>>(|| None);
    let last_row = rows.len();
    let col_count_now = headers.len();
    let last_col = col_count_now.saturating_sub(1);

    // Rendered unconditionally (review, 2026-10-02 §3.2): if the toolbar
    // only appeared once a cell took focus, the table would shift down by
    // its height on the user's first click, under their own pointer. Its
    // buttons are disabled until a cell has focus instead.
    let current = *focused.read();
    let toolbar_field_id = current
        .map(|(row, col)| format!("{field_id}-{row}-{col}"))
        .unwrap_or_else(|| field_id.clone());

    rsx! {
        div { class: "table-block",
            InlineToolbar {
                field_id: toolbar_field_id,
                block_id,
                revision,
                lang,
                cell: current,
                enabled: current.is_some(),
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
                                    aria_label: column_name(header, ci, lang),
                                    style: "text-align: {text_align_at(&alignments, ci)};",
                                    onfocus: move |_| focused.set(Some((0, ci))),
                                    onkeydown: {
                                        let field_id = field_id.clone();
                                        move |event: KeyboardEvent| {
                                            cell_keydown(&event, &field_id, (0, ci), (last_row, last_col))
                                        }
                                    },
                                    onchange: move |evt: Event<FormData>| dispatch(
                                        state,
                                        revision,
                                        block_id,
                                        toasts,
                                        lang,
                                        FormBlockEdit::ReplaceTableCell { row: 0, col: ci, text: evt.value() },
                                    ),
                                }
                                ColumnActionsMenu {
                                    field_id: field_id.clone(),
                                    block_id,
                                    revision,
                                    toasts,
                                    lang,
                                    col: ci,
                                    count: col_count_now,
                                    alignment: alignment_at(&alignments, ci),
                                    label: column_name(header, ci, lang),
                                    open: open_col_menu,
                                    entry: col_menu_entry,
                                }
                            }
                        }
                        th { class: "table-row-actions-header" }
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
                                        style: "text-align: {text_align_at(&alignments, ci)};",
                                        // Column header's plain text plus row
                                        // number (RFC-048 §5.4, slice 5 §2.3),
                                        // e.g. "Name, row 2".
                                        aria_label: {
                                            let header = headers.get(ci).map(String::as_str).unwrap_or("");
                                            tr(lang, "table.cell_label")
                                                .replacen("{}", &column_name(header, ci, lang), 1)
                                                .replacen("{}", &(ri + 1).to_string(), 1)
                                        },
                                        onfocus: move |_| focused.set(Some((ri + 1, ci))),
                                        onkeydown: {
                                            let field_id = field_id.clone();
                                            move |event: KeyboardEvent| {
                                                cell_keydown(&event, &field_id, (ri + 1, ci), (last_row, last_col))
                                            }
                                        },
                                        onchange: {
                                            let row_num = ri + 1;
                                            move |evt: Event<FormData>| dispatch(
                                                state,
                                                revision,
                                                block_id,
                                                toasts,
                                                lang,
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
                            td { class: "table-row-actions-cell",
                                RowActionsMenu {
                                    field_id: field_id.clone(),
                                    block_id,
                                    revision,
                                    toasts,
                                    lang,
                                    row: ri + 1,
                                    last_row,
                                    open: open_row_menu,
                                    entry: row_menu_entry,
                                }
                            }
                        }
                    }
                }
            }
            button {
                id: "{field_id}-add-row",
                class: "table-add-row",
                onclick: move |_| dispatch(state, revision, block_id, toasts, lang, FormBlockEdit::AddTableRow),
                AddIcon {}
                {tr(lang, "table.add_row")}
            }
            // RFC-048 slice 5 §2.1: only ever on request. On a table that is
            // already tidy it does nothing at all: `dispatch` drops it before
            // it becomes an edit.
            button {
                id: "{field_id}-tidy",
                class: "table-add-row table-tidy",
                onclick: move |_| dispatch(state, revision, block_id, toasts, lang, FormBlockEdit::TidyTable),
                {tr(lang, "table.tidy")}
            }
            // Warn if col_count == 0 (degenerate table).
            if col_count == 0 {
                p { class: "muted", {tr(lang, "table.empty")} }
            }
        }
    }
}

/// A column's name for labels (RFC-048 slice 5 §2.3): its header's plain text,
/// so `**Name**` reads "Name", or "Column 3" when the header is empty.
fn column_name(header: &str, col: usize, lang: Lang) -> String {
    let plain = cell_plain_text(header);
    if plain.is_empty() {
        tr(lang, "table.column_fallback").replacen("{}", &(col + 1).to_string(), 1)
    } else {
        plain
    }
}

/// What each menu item of [`RowActionsMenu`] does, and the focus it
/// leaves behind (RFC-048 slice 3 §2.2): a plain function, not a
/// closure captured once and reused, so there is no question of which
/// copy of `field_id` an event handler owns -- every `onclick` below
/// clones `field_id` for its own call.
///
/// Bundles `dispatch`'s own five non-edit arguments (task 056 §2.1
/// widened it to carry `toasts`/`lang`, which pushed every caller past
/// clippy's argument-count limit) -- every field is `Copy`, so this is
/// itself just a `Copy` struct, not a lifetime to thread through.
#[derive(Clone, Copy)]
pub(super) struct EditContext {
    pub(super) state: Signal<AppState>,
    pub(super) revision: u64,
    pub(super) block_id: BlockId,
    pub(super) toasts: Signal<Vec<Toast>>,
    pub(super) lang: Lang,
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn a_header_is_named_by_its_plain_text() {
        assert_eq!(column_name("**Name**", 0, Lang::En), "Name");
        assert_eq!(column_name("`id` _key_", 1, Lang::En), "id key");
    }

    #[test]
    fn an_empty_header_is_named_by_its_position() {
        assert_eq!(column_name("", 2, Lang::En), "Column 3");
        assert_eq!(column_name("  ", 0, Lang::En), "Column 1");
        assert_eq!(column_name("", 2, Lang::Ja), "列 3");
    }

    #[test]
    fn a_data_cell_label_names_its_column_and_row() {
        let label = tr(Lang::En, "table.cell_label")
            .replacen("{}", &column_name("**Name**", 0, Lang::En), 1)
            .replacen("{}", "2", 1);
        assert_eq!(label, "Name, row 2");
        let empty = tr(Lang::En, "table.cell_label")
            .replacen("{}", &column_name("", 2, Lang::En), 1)
            .replacen("{}", "2", 1);
        assert_eq!(empty, "Column 3, row 2");
    }
}
