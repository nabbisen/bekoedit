//! Simple GFM table grid (RFC-027, extended by RFC-048 slice 2).
//!
//! Its own component, not inlined into `block_view.rs`'s match, so the
//! focused-cell signal below is only ever created while a table block is
//! actually mounted -- Dioxus hooks must be called in the same order on
//! every render of a component, so a signal cannot live inside one match
//! arm of a component that renders every block kind through one big match.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, TableRowDirection, TableRowPosition, fingerprint::BlockId};

use super::dispatch;
use super::inline_toolbar::InlineToolbar;
use crate::components::icons::AddIcon;
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus::{self, FocusMove};

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
    let last_row = rows.len();

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
                                    aria_label: "{header}",
                                    onfocus: move |_| focused.set(Some((0, ci))),
                                    onchange: move |evt: Event<FormData>| dispatch(
                                        state,
                                        revision,
                                        block_id,
                                        toasts,
                                        lang,
                                        FormBlockEdit::ReplaceTableCell { row: 0, col: ci, text: evt.value() },
                                    ),
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
            // Warn if col_count == 0 (degenerate table).
            if col_count == 0 {
                p { class: "muted", {tr(lang, "table.empty")} }
            }
        }
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
struct EditContext {
    state: Signal<AppState>,
    revision: u64,
    block_id: BlockId,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
}

fn insert_row(ctx: EditContext, field_id: &str, at: usize, position: TableRowPosition) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::InsertTableRow { at, position },
    );
    // The new row's first cell: there is new, empty content the user
    // will likely type into right away.
    let new_row = match position {
        TableRowPosition::Above => at,
        TableRowPosition::Below => at + 1,
    };
    shell_focus::focus_table_cell(field_id, new_row, 0);
}

fn delete_row(ctx: EditContext, field_id: &str, row: usize, last_row: usize) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::DeleteTableRow { row },
    );
    // The row that took the deleted row's place, or the previous one if
    // the deleted row was last; falling back to the table's own "Add
    // row" button if that previous row would be the header, which has
    // no actions button of its own.
    if row < last_row {
        shell_focus::focus_table_row_actions(field_id, row);
    } else if row > 1 {
        shell_focus::focus_table_row_actions(field_id, row - 1);
    } else {
        shell_focus::focus_table_add_row_button(field_id);
    }
}

fn move_row(ctx: EditContext, field_id: &str, row: usize, direction: TableRowDirection) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::MoveTableRow { row, direction },
    );
    // The moved row's own new position.
    let new_row = match direction {
        TableRowDirection::Up => row - 1,
        TableRowDirection::Down => row + 1,
    };
    shell_focus::focus_table_row_actions(field_id, new_row);
}

/// One data row's actions button and its menu (RFC-048 slice 3 §2.2):
/// insert above/below, delete, move up/down. `open`/`entry` are shared
/// with every other row's own `RowActionsMenu` in the same table, so
/// opening one closes any other and there is one keyboard-entry intent
/// for the whole table, not one per row.
#[component]
fn RowActionsMenu(
    field_id: String,
    block_id: BlockId,
    revision: u64,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
    row: usize,
    last_row: usize,
    open: Signal<Option<usize>>,
    entry: Signal<Option<FocusMove>>,
) -> Element {
    let mut open = open;
    let mut entry = entry;
    let state = use_context::<Signal<AppState>>();
    let ctx = EditContext {
        state,
        revision,
        block_id,
        toasts,
        lang,
    };
    let is_open = *open.read() == Some(row);
    let menu_id = format!("{field_id}-row-menu-{row}");
    let trigger_id = format!("{field_id}-row-actions-{row}");

    rsx! {
        div {
            class: "table-row-actions-wrap",
            onclick: move |event| event.stop_propagation(),
            onkeydown: {
                let field_id = field_id.clone();
                move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        open.set(None);
                        shell_focus::focus_table_row_actions(&field_id, row);
                        return;
                    }
                    if let Some(target) = shell_focus::menu_item_key_intent(&event.key()) {
                        event.prevent_default();
                        shell_focus::focus_table_row_menu_item(&field_id, row, target);
                    }
                }
            },
            button {
                id: "{trigger_id}",
                class: if is_open { "icon-btn table-row-actions-btn active" } else { "icon-btn table-row-actions-btn" },
                aria_label: tr(lang, "table.row_actions").replacen("{}", &row.to_string(), 1),
                aria_haspopup: "menu",
                aria_expanded: "{is_open}",
                aria_controls: "{menu_id}",
                onclick: move |_| {
                    if is_open {
                        open.set(None);
                    } else {
                        open.set(Some(row));
                    }
                },
                onkeydown: {
                    let field_id = field_id.clone();
                    move |event: KeyboardEvent| {
                        let Some(target) = shell_focus::trigger_key_intent(&event.key()) else {
                            return;
                        };
                        event.prevent_default();
                        event.stop_propagation();
                        if is_open {
                            shell_focus::focus_table_row_menu_item(&field_id, row, target);
                        } else {
                            entry.set(Some(target));
                            open.set(Some(row));
                        }
                    }
                },
                "⋮"
            }
            if is_open {
                div {
                    id: "{menu_id}",
                    role: "menu",
                    tabindex: "-1",
                    class: "table-row-actions-menu",
                    onmounted: {
                        let field_id = field_id.clone();
                        move |_| {
                            let pending = *entry.peek();
                            if let Some(target) = pending {
                                entry.set(None);
                                shell_focus::focus_table_row_menu_item(&field_id, row, target);
                            }
                        }
                    },
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                insert_row(ctx, &field_id, row, TableRowPosition::Above);
                            }
                        },
                        {tr(lang, "table.row.insert_above")}
                    }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                insert_row(ctx, &field_id, row, TableRowPosition::Below);
                            }
                        },
                        {tr(lang, "table.row.insert_below")}
                    }
                    hr { class: "dropdown-sep" }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                delete_row(ctx, &field_id, row, last_row);
                            }
                        },
                        {tr(lang, "table.row.delete")}
                    }
                    hr { class: "dropdown-sep" }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        disabled: row <= 1,
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                move_row(ctx, &field_id, row, TableRowDirection::Up);
                            }
                        },
                        {tr(lang, "table.row.move_up")}
                    }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        disabled: row >= last_row,
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                move_row(ctx, &field_id, row, TableRowDirection::Down);
                            }
                        },
                        {tr(lang, "table.row.move_down")}
                    }
                }
            }
        }
    }
}
