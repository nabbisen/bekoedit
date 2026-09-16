//! Keyboard entry into an overflow menu (RFC-042 §7.2, task 017).
//!
//! Enter, Space or Down on a trigger must open the menu **and** focus its
//! first item; Up focuses the last. The items do not exist until Dioxus has
//! rendered the open state, so the focus move is driven by the menu container
//! being mounted (`onmounted`), never by time passing. A frame count, a
//! deadline or a poll only makes losing that race less likely; the mount
//! event is the render, so there is no race to lose.
//!
//! Shared by the app menu and the editor-tools menu, so the two cannot drift.

use dioxus::prelude::*;

use crate::shell_focus::{self, FocusMove};

/// The trigger's keyboard path. A closed menu records `target` as its entry
/// intent, then opens; the container's `onmounted` consumes the intent. An
/// already-open menu gets no mount event, but its items exist, so focus moves
/// directly.
pub fn enter_menu_by_key(
    mut intent: Signal<Option<FocusMove>>,
    already_open: bool,
    menu_id: &'static str,
    target: FocusMove,
    open_menu: impl FnOnce(),
) {
    if already_open {
        shell_focus::focus_menu_item(menu_id, target);
    } else {
        intent.set(Some(target));
        open_menu();
    }
}

/// The menu container's `onmounted`: use the entry intent once and clear it.
/// A mouse open records no intent, so it leaves focus where it is.
pub fn consume_menu_entry(mut intent: Signal<Option<FocusMove>>, menu_id: &'static str) {
    let pending = *intent.peek();
    if let Some(target) = pending {
        intent.set(None);
        shell_focus::focus_menu_item(menu_id, target);
    }
}
