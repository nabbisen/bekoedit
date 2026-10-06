//! Simple GFM table grid (RFC-027, extended by RFC-048 slice 2).
//!
//! Its own component, not inlined into `block_view.rs`'s match, so the
//! focused-cell signal below is only ever created while a table block is
//! actually mounted -- Dioxus hooks must be called in the same order on
//! every render of a component, so a signal cannot live inside one match
//! arm of a component that renders every block kind through one big match.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{
    FormBlockEdit, TableAlignment, TableColumnDirection, TableColumnPosition, TableRowDirection,
    TableRowPosition, fingerprint::BlockId,
};

use super::dispatch;
use super::inline_toolbar::InlineToolbar;
use crate::components::icons::AddIcon;
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus::{self, FocusMove};
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
                                    style: "text-align: {text_align_at(&alignments, ci)};",
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
                                ColumnActionsMenu {
                                    field_id: field_id.clone(),
                                    block_id,
                                    revision,
                                    toasts,
                                    lang,
                                    col: ci,
                                    count: col_count_now,
                                    alignment: alignment_at(&alignments, ci),
                                    label: cell_plain_text(header),
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
    let menu_id_for_place = menu_id.clone();
    let trigger_id_for_place = trigger_id.clone();

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
                            document::eval(&super::placement::place_menu_script(&menu_id_for_place, &trigger_id_for_place));
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

/// CSS `text-align` for a column's cells (RFC-048 slice 4 §2.2).
fn text_align(alignment: TableAlignment) -> &'static str {
    match alignment {
        TableAlignment::None => "start",
        TableAlignment::Left => "left",
        TableAlignment::Centre => "center",
        TableAlignment::Right => "right",
    }
}

fn alignment_at(alignments: &[TableAlignment], col: usize) -> TableAlignment {
    alignments.get(col).copied().unwrap_or(TableAlignment::None)
}

fn text_align_at(alignments: &[TableAlignment], col: usize) -> &'static str {
    text_align(alignment_at(alignments, col))
}

/// Where focus goes after a column is deleted, given the column count left
/// (RFC-048 slice 4 §2.2): the column that took its place, or the previous one
/// if the deleted column was last. `None` when no column is left.
fn column_after_delete(col: usize, count_after: usize) -> Option<usize> {
    match count_after {
        0 => None,
        _ if col < count_after => Some(col),
        _ => Some(col - 1),
    }
}

fn insert_column(ctx: EditContext, field_id: &str, col: usize, position: TableColumnPosition) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::InsertTableColumn { col, position },
    );
    // The new column's header cell: the user will likely name it right away.
    let new_col = match position {
        TableColumnPosition::Left => col,
        TableColumnPosition::Right => col + 1,
    };
    shell_focus::focus_table_cell(field_id, 0, new_col);
}

fn delete_column(ctx: EditContext, field_id: &str, col: usize, count: usize) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::DeleteTableColumn { col },
    );
    if let Some(target) = column_after_delete(col, count - 1) {
        shell_focus::focus_table_column_actions(field_id, target);
    }
}

fn move_column(ctx: EditContext, field_id: &str, col: usize, direction: TableColumnDirection) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::MoveTableColumn { col, direction },
    );
    let new_col = match direction {
        TableColumnDirection::Left => col - 1,
        TableColumnDirection::Right => col + 1,
    };
    shell_focus::focus_table_column_actions(field_id, new_col);
}

fn set_column_alignment(ctx: EditContext, field_id: &str, col: usize, alignment: TableAlignment) {
    dispatch(
        ctx.state,
        ctx.revision,
        ctx.block_id,
        ctx.toasts,
        ctx.lang,
        FormBlockEdit::SetTableColumnAlignment { col, alignment },
    );
    shell_focus::focus_table_column_actions(field_id, col);
}

/// One column header's actions button and its menu (RFC-048 slice 4 §2.2):
/// insert left and right, delete, move, and alignment. The menu is fixed and
/// placed by the same script as a row's menu (task 058), with the same keyboard
/// rules, and one column's menu is open at a time.
#[component]
fn ColumnActionsMenu(
    field_id: String,
    block_id: BlockId,
    revision: u64,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
    col: usize,
    count: usize,
    alignment: TableAlignment,
    label: String,
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
    let is_open = *open.read() == Some(col);
    let menu_id = format!("{field_id}-col-menu-{col}");
    let trigger_id = format!("{field_id}-col-actions-{col}");
    let menu_id_for_place = menu_id.clone();
    let trigger_id_for_place = trigger_id.clone();
    let current = |item: TableAlignment| item == alignment;

    rsx! {
        div {
            class: "table-row-actions-wrap",
            onclick: move |event| event.stop_propagation(),
            onkeydown: {
                let field_id = field_id.clone();
                move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        open.set(None);
                        shell_focus::focus_table_column_actions(&field_id, col);
                        return;
                    }
                    if let Some(target) = shell_focus::menu_item_key_intent(&event.key()) {
                        event.prevent_default();
                        shell_focus::focus_table_column_menu_item(&field_id, col, target);
                    }
                }
            },
            button {
                id: "{trigger_id}",
                class: if is_open { "icon-btn table-col-actions-btn active" } else { "icon-btn table-col-actions-btn" },
                aria_label: tr(lang, "table.column_actions").replacen("{}", &label, 1),
                aria_haspopup: "menu",
                aria_expanded: "{is_open}",
                aria_controls: "{menu_id}",
                onclick: move |_| {
                    if is_open {
                        open.set(None);
                    } else {
                        open.set(Some(col));
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
                            shell_focus::focus_table_column_menu_item(&field_id, col, target);
                        } else {
                            entry.set(Some(target));
                            open.set(Some(col));
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
                    class: "table-row-actions-menu table-col-menu",
                    onmounted: {
                        let field_id = field_id.clone();
                        move |_| {
                            document::eval(&super::placement::place_menu_script(&menu_id_for_place, &trigger_id_for_place));
                            let pending = *entry.peek();
                            if let Some(target) = pending {
                                entry.set(None);
                                shell_focus::focus_table_column_menu_item(&field_id, col, target);
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
                                insert_column(ctx, &field_id, col, TableColumnPosition::Left);
                            }
                        },
                        {tr(lang, "table.col.insert_left")}
                    }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                insert_column(ctx, &field_id, col, TableColumnPosition::Right);
                            }
                        },
                        {tr(lang, "table.col.insert_right")}
                    }
                    hr { class: "dropdown-sep" }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        disabled: count <= 1,
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                delete_column(ctx, &field_id, col, count);
                            }
                        },
                        {tr(lang, "table.col.delete")}
                    }
                    hr { class: "dropdown-sep" }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        disabled: col == 0,
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                move_column(ctx, &field_id, col, TableColumnDirection::Left);
                            }
                        },
                        {tr(lang, "table.col.move_left")}
                    }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        disabled: col + 1 >= count,
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                move_column(ctx, &field_id, col, TableColumnDirection::Right);
                            }
                        },
                        {tr(lang, "table.col.move_right")}
                    }
                    hr { class: "dropdown-sep" }
                    button {
                        class: "dropdown-item",
                        role: "menuitem",
                        tabindex: "-1",
                        aria_current: "{current(TableAlignment::None)}",
                        class: if current(TableAlignment::None) { "dropdown-item active" } else { "dropdown-item" },
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                set_column_alignment(ctx, &field_id, col, TableAlignment::None);
                            }
                        },
                        {tr(lang, "table.align.none")}
                    }
                    button {
                        class: if current(TableAlignment::Left) { "dropdown-item active" } else { "dropdown-item" },
                        role: "menuitem",
                        tabindex: "-1",
                        aria_current: "{current(TableAlignment::Left)}",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                set_column_alignment(ctx, &field_id, col, TableAlignment::Left);
                            }
                        },
                        {tr(lang, "table.align.left")}
                    }
                    button {
                        class: if current(TableAlignment::Centre) { "dropdown-item active" } else { "dropdown-item" },
                        role: "menuitem",
                        tabindex: "-1",
                        aria_current: "{current(TableAlignment::Centre)}",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                set_column_alignment(ctx, &field_id, col, TableAlignment::Centre);
                            }
                        },
                        {tr(lang, "table.align.centre")}
                    }
                    button {
                        class: if current(TableAlignment::Right) { "dropdown-item active" } else { "dropdown-item" },
                        role: "menuitem",
                        tabindex: "-1",
                        aria_current: "{current(TableAlignment::Right)}",
                        onclick: {
                            let field_id = field_id.clone();
                            move |_| {
                                open.set(None);
                                set_column_alignment(ctx, &field_id, col, TableAlignment::Right);
                            }
                        },
                        {tr(lang, "table.align.right")}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;

    /// Task 058-style pure decision (RFC-048 slice 4 §2.2): after a delete,
    /// focus lands on the column that took the deleted one's place, or the
    /// previous one when it was last; nothing when no column is left.
    #[test]
    fn focus_after_delete_is_the_column_that_took_its_place_or_the_previous() {
        assert_eq!(column_after_delete(1, 3), Some(1));
        assert_eq!(column_after_delete(2, 2), Some(1));
        assert_eq!(column_after_delete(0, 0), None);
    }

    #[test]
    fn each_alignment_maps_to_its_css_text_align() {
        assert_eq!(text_align(TableAlignment::Centre), "center");
        assert_eq!(text_align(TableAlignment::Left), "left");
        assert_eq!(text_align(TableAlignment::Right), "right");
        assert_eq!(text_align(TableAlignment::None), "start");
    }
}
