//! A table column header's actions menu, and the alignment helpers the grid
//! uses (RFC-048 slice 4), moved out of `table_view.rs` unchanged (task 059).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{
    FormBlockEdit, TableAlignment, TableColumnDirection, TableColumnPosition, fingerprint::BlockId,
};

use super::dispatch;
use super::table_view::EditContext;
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus::{self, FocusMove};

/// CSS `text-align` for a column's cells (RFC-048 slice 4 §2.2).
pub(super) fn text_align(alignment: TableAlignment) -> &'static str {
    match alignment {
        TableAlignment::None => "start",
        TableAlignment::Left => "left",
        TableAlignment::Centre => "center",
        TableAlignment::Right => "right",
    }
}

pub(super) fn alignment_at(alignments: &[TableAlignment], col: usize) -> TableAlignment {
    alignments.get(col).copied().unwrap_or(TableAlignment::None)
}

pub(super) fn text_align_at(alignments: &[TableAlignment], col: usize) -> &'static str {
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
pub(super) fn ColumnActionsMenu(
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
