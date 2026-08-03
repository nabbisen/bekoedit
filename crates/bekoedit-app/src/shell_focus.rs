//! Shell-side focus-restore helper (RFC-042 slice 1, handoff §7.2).
//!
//! Trigger ids are static constants, never derived from user input — the
//! restore script below only ever embeds one of these.

use dioxus::prelude::*;

pub const TRIGGER_APP_MENU: &str = "app-menu-trigger";
pub const TRIGGER_EDITOR_TOOLS: &str = "editor-tools-trigger";
pub const TRIGGER_WORKSPACE_SEARCH: &str = "workspace-search-trigger";
pub const TRIGGER_NEW_FILE: &str = "workspace-new-file-trigger";

/// Focus an element by id on the next animation frame. A missing element is
/// a no-op, never a panic.
///
/// `id` is `&'static str`, not `&str`: every current caller passes one of
/// the constants above, and slice 2 introduces user-controlled row ids that
/// must never reach this function unsanitized. The type keeps that true by
/// construction instead of by convention (review recommendation R5).
pub fn focus_element(id: &'static str) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.getElementById('{id}')?.focus())"#,
    ));
}

/// Focus the nth element matching `[data-tree-row]`, on the next frame. A
/// missing or out-of-range element is a no-op, never a panic.
///
/// `index` is a `usize`, not a path-derived string (RFC-042 slice 2 handoff
/// §7.5/§12): only an integer is interpolated, so there is no caller-
/// controlled text for this script to carry, and nothing to sanitize.
pub fn focus_tree_row(index: usize) {
    document::eval(&format!(
        r#"requestAnimationFrame(() => document.querySelectorAll('[data-tree-row]')[{index}]?.focus())"#,
    ));
}
