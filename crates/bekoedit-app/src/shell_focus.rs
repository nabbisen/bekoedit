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
