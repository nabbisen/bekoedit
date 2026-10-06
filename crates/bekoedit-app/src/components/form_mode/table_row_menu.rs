//! A table data row's actions menu (RFC-048 slice 3), moved out of
//! `table_view.rs` unchanged (task 059).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, TableRowDirection, TableRowPosition, fingerprint::BlockId};

use super::dispatch;
use super::table_view::EditContext;
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus::{self, FocusMove};

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
pub(super) fn RowActionsMenu(
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
                        match event.key() {
                            Key::ArrowRight => {
                                event.prevent_default();
                                shell_focus::focus_table_trigger_step(1);
                                return;
                            }
                            Key::ArrowLeft => {
                                event.prevent_default();
                                shell_focus::focus_table_trigger_step(-1);
                                return;
                            }
                            _ => {}
                        }
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
