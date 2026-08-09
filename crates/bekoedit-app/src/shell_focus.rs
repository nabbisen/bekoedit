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
/// from what is actually rendered (RFC-042 slice 3 handoff §5.3). A missing
/// menu or empty item list is a no-op, never a panic.
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
