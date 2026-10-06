//! Shell-side focus-restore helper (RFC-042 slice 1, handoff §7.2).
//!
//! Trigger ids are static constants, never derived from user input — the
//! restore script below only ever embeds one of these.

use dioxus::prelude::*;

pub const TRIGGER_APP_MENU: &str = "app-menu-trigger";
pub const TRIGGER_EDITOR_TOOLS: &str = "editor-tools-trigger";
pub const TRIGGER_WORKSPACE_SEARCH: &str = "workspace-search-trigger";
pub const TRIGGER_NEW_FILE: &str = "workspace-new-file-trigger";
/// The app bar's home/logo button (RFC-042 slice 4, handoff §5.2) — the one
/// control guaranteed present in whatever screen replaces Recovery on exit
/// (MainShell or StartScreen both render the app bar above their own
/// content), so it stands in for "the next screen's natural first control"
/// rather than an invented phantom trigger.
pub const TRIGGER_APP_LOGO: &str = "app-bar-logo-trigger";

/// Focus an element by id on the next animation frame. A missing element is
/// a no-op, never a panic.
///
/// `id` is `&'static str`, not `&str`: every current caller passes one of
/// the constants above, and slice 2 introduces user-controlled row ids that
/// must never reach this function unsanitized. The type keeps that true by
/// construction instead of by convention (review recommendation R5).
pub fn focus_element(id: &'static str) {
    document::eval(&focus_element_script(id));
}

fn focus_element_script(id: &'static str) -> String {
    format!(r#"requestAnimationFrame(() => document.getElementById('{id}')?.focus())"#)
}

/// Focus the nth element matching `[data-tree-row]`, on the next frame. A
/// missing or out-of-range element is a no-op, never a panic.
///
/// `index` is a `usize`, not a path-derived string (RFC-042 slice 2 handoff
/// §7.5/§12): only an integer is interpolated, so there is no caller-
/// controlled text for this script to carry, and nothing to sanitize.
pub fn focus_tree_row(index: usize) {
    document::eval(&focus_tree_row_script(index));
}

fn focus_tree_row_script(index: usize) -> String {
    format!(
        r#"requestAnimationFrame(() => document.querySelectorAll('[data-tree-row]')[{index}]?.focus())"#,
    )
}

/// Stable handles the RFC-044 second run clicks by (slice 3 handoff §6.1).
/// The Settings item is the first click the harness makes inside the app
/// menu, whose neighbours open native dialogs that escape xvfb; neither may
/// be found by position or translated label.
pub const MENU_ITEM_APP_SETTINGS: &str = "app-menu-settings";
pub const BUTTON_SETTINGS_CLOSE: &str = "settings-close";

/// The two overflow-menu container ids (RFC-042 slice 3, handoff §5.7).
pub const MENU_APP_OVERFLOW: &str = "app-overflow-menu";
pub const MENU_EDITOR_TOOLS: &str = "editor-tools-menu";

/// The mode tablist container id.
pub const TABLIST_MODE_SWITCH: &str = "editor-mode-switch";

/// Where a roving-focus move should land. Shared by menus and the mode
/// tablist — both move among a set of DOM siblings the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMove {
    First,
    Last,
    Next,
    Previous,
}

/// Move focus among `menu_id`'s `[role="menuitem"]` descendants, on the next
/// frame. The item set is read from the DOM at the moment of use, not
/// mirrored in Rust: item counts vary at runtime (a conditional item behind
/// `has_workspace`/`backlinks_available`), so a Rust-side list could drift
/// from what is actually rendered (RFC-042 slice 3 handoff §5.3).
///
/// A missing menu or empty item list is a no-op. That is for moves inside a
/// menu that is **already open** -- a stray key after the menu has closed
/// must do nothing. **Entry into a menu that is just opening must not rely on
/// it**: that menu's items may not be rendered yet, and the move would be
/// silently lost, which is exactly how keyboard entry to both menus failed
/// until task 017. Opening by keyboard goes through `menu_entry`, whose focus
/// move runs from the container's `onmounted`.
pub fn focus_menu_item(menu_id: &'static str, position: FocusMove) {
    document::eval(&focus_menu_item_script(menu_id, position));
}

fn focus_menu_item_script(menu_id: &'static str, position: FocusMove) -> String {
    let target = focus_move_expr("items", position);
    format!(
        r#"requestAnimationFrame(() => {{
            const menu = document.getElementById('{menu_id}');
            const items = menu ? [...menu.querySelectorAll('[role="menuitem"]')] : [];
            if (items.length === 0) return;
            const current = items.indexOf(document.activeElement);
            {target}?.focus();
        }});"#,
    )
}

/// Move focus among the mode tablist's `[role="tab"]` descendants, on the
/// next frame. DOM-relative for the same reason as `focus_menu_item`: the
/// tab carrying `tabindex="0"` tracks the *selected* mode, not wherever
/// arrow navigation last landed, so the current position can only be read
/// from `document.activeElement`, not from Rust-known state.
pub fn focus_tab(position: FocusMove) {
    document::eval(&focus_tab_script(position));
}

fn focus_tab_script(position: FocusMove) -> String {
    let target = focus_move_expr("tabs", position);
    format!(
        r#"requestAnimationFrame(() => {{
            const list = document.getElementById('{TABLIST_MODE_SWITCH}');
            const tabs = list ? [...list.querySelectorAll('[role="tab"]')] : [];
            if (tabs.length === 0) return;
            const current = tabs.indexOf(document.activeElement);
            {target}?.focus();
        }});"#,
    )
}

fn focus_move_expr(array: &'static str, position: FocusMove) -> String {
    match position {
        FocusMove::First => format!("{array}[0]"),
        FocusMove::Last => format!("{array}[{array}.length - 1]"),
        FocusMove::Next => format!("{array}[(current + 1) % {array}.length]"),
        FocusMove::Previous => format!("{array}[(current - 1 + {array}.length) % {array}.length]"),
    }
}

/// Focuses one table cell's input on the next frame (RFC-048 slice 3
/// handoff §2.2, after an insert). `field_id` is the table's own id
/// (`fb-{ordinal}-{content_hash}`, built entirely from internal
/// identifiers, never user text -- the same reasoning as
/// [`focus_element`]'s `&'static str`, relaxed to `&str` here only
/// because the caller-supplied part is always that already-safe id plus
/// plain `usize`s, never arbitrary text). A missing element is a no-op.
pub fn focus_table_cell(field_id: &str, row: usize, col: usize) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.getElementById('{field_id}-{row}-{col}')?.focus())"#,
    ));
}

/// Focuses a table's own "Add row" button on the next frame (RFC-048
/// slice 3 handoff §2.2: a delete's fallback when there is no previous
/// data row's actions button to land on). Same id-safety reasoning as
/// [`focus_table_cell`].
pub fn focus_table_add_row_button(field_id: &str) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.getElementById('{field_id}-add-row')?.focus())"#,
    ));
}

/// Focuses one row's own actions-menu trigger button on the next frame
/// (RFC-048 slice 3 handoff §2.2, after a delete or a move). Same
/// id-safety reasoning as [`focus_table_cell`].
pub fn focus_table_row_actions(field_id: &str, row: usize) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.getElementById('{field_id}-row-actions-{row}')?.focus())"#,
    ));
}

/// Move focus among one row-actions menu's `[role="menuitem"]`
/// descendants, on the next frame -- the per-row analogue of
/// [`focus_menu_item`], needed because each table row has its own menu
/// container id (`{field_id}-row-menu-{row}`), not one of the fixed
/// `MENU_*` constants.
pub fn focus_table_row_menu_item(field_id: &str, row: usize, position: FocusMove) {
    let target = focus_move_expr("items", position);
    document::eval(&format!(
        r#"requestAnimationFrame(() => {{
            const menu = document.getElementById('{field_id}-row-menu-{row}');
            const items = menu ? [...menu.querySelectorAll('[role="menuitem"]')] : [];
            if (items.length === 0) return;
            const current = items.indexOf(document.activeElement);
            {target}?.focus();
        }});"#,
    ));
}

/// What a keydown on an overflow-menu **trigger** means (RFC-042 §7.2,
/// handoff §5.2). `None` means the key does nothing at the trigger — in
/// particular, Enter/Space's native button-click activation already opens
/// the menu; this only adds the "and focus the first/last item" half.
pub fn trigger_key_intent(key: &Key) -> Option<FocusMove> {
    match key {
        Key::ArrowDown | Key::Enter => Some(FocusMove::First),
        Key::Character(space) if space == " " => Some(FocusMove::First),
        Key::ArrowUp => Some(FocusMove::Last),
        _ => None,
    }
}

/// Focuses one column's own actions-menu trigger on the next frame (RFC-048
/// slice 4 §2.2). Same id-safety reasoning as [`focus_table_row_actions`].
pub fn focus_table_column_actions(field_id: &str, col: usize) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.getElementById('{field_id}-col-actions-{col}')?.focus())"#,
    ));
}

/// Move focus among one column menu's `[role="menuitem"]` descendants, on the
/// next frame -- the column analogue of [`focus_table_row_menu_item`].
pub fn focus_table_column_menu_item(field_id: &str, col: usize, position: FocusMove) {
    let target = focus_move_expr("items", position);
    document::eval(&format!(
        r#"requestAnimationFrame(() => {{
            const menu = document.getElementById('{field_id}-col-menu-{col}');
            const items = menu ? [...menu.querySelectorAll('[role="menuitem"]')] : [];
            if (items.length === 0) return;
            const current = items.indexOf(document.activeElement);
            {target}?.focus();
        }});"#,
    ));
}

/// Moves focus one step along a table's actions buttons (every column's, then
/// every row's, in document order), from whichever one has focus (RFC-048
/// slice 5 §2.2). It is how the row buttons stay reachable by keyboard now
/// that Tab moves between cells. `delta` is 1 or -1; the ends do not wrap.
pub fn focus_table_trigger_step(delta: i32) {
    document::eval(&format!(
        r#"(() => {{
            const here = document.activeElement;
            const block = here && here.closest ? here.closest('.table-block') : null;
            if (!block) return;
            const all = [...block.querySelectorAll('.table-col-actions-btn, .table-row-actions-btn')];
            const next = all[all.indexOf(here) + ({delta})];
            if (next) next.focus();
        }})();"#,
    ));
}

/// What a keydown on a focused **menu item** means, once the menu is open
/// (handoff §5.2). Enter/Space are deliberately absent — native
/// button-click activation already handles them via each item's own
/// `onclick`.
pub fn menu_item_key_intent(key: &Key) -> Option<FocusMove> {
    match key {
        Key::ArrowDown => Some(FocusMove::Next),
        Key::ArrowUp => Some(FocusMove::Previous),
        Key::Home => Some(FocusMove::First),
        Key::End => Some(FocusMove::Last),
        _ => None,
    }
}

/// What a keydown on a focused **mode tab** means (handoff §5.5). Enter/Space
/// are absent for the same reason as menu items: native activation already
/// calls the tab's own `onclick`, and RFC-042 §7.3 requires manual (not
/// automatic) mode activation, so arrow keys must never trigger it.
pub fn tab_key_intent(key: &Key) -> Option<FocusMove> {
    match key {
        Key::ArrowRight => Some(FocusMove::Next),
        Key::ArrowLeft => Some(FocusMove::Previous),
        Key::Home => Some(FocusMove::First),
        Key::End => Some(FocusMove::Last),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
