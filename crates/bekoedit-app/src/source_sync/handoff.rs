//! Handoff activation (RFC-042 §6.2 rule 3, amended 2026-09-15; task 016).
//!
//! An item inside a shell surface whose own action is a source-focus
//! interaction -- a search result, App menu "New File", editor-tools "Split".
//! Explicit dismissal would restore focus to the surface's trigger; the user
//! chose the item to go somewhere else, so a handoff instead:
//!
//! 1. releases shell authority **before** the interaction is allocated, so
//!    the allocation is not refused (§6.2 rule 2);
//! 2. **never restores** focus to the trigger;
//! 3. closes the surface's DOM in the launch finalizer, so the activated
//!    item stays connected as the guard's origin until the guard has armed;
//! 4. leaves focus to the source controller (rule 4). No timer (§6.4).
//!
//! Every other close path -- Escape, the trigger's toggle, items whose action
//! is not a source-focus interaction -- keeps releasing and restoring.

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::components::toast::Toast;

use super::{SourceCommand, SourceInteractionOrigin, SourceSyncState, submit_source_interaction};

/// Step 1, kept apart so the ordering is testable without a WebView.
fn release_for_handoff(sync: &mut SourceSyncState) {
    sync.release_shell_focus();
}

/// Activates a handoff item. `close_surface` closes the shell surface's DOM
/// and runs as the launch finalizer, never before the guard has armed.
pub fn submit_handoff_activation(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    close_surface: impl FnOnce() + 'static,
) {
    release_for_handoff(&mut sync.write());
    submit_source_interaction(sync, state, mode, toasts, command, origin, close_surface);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_sync::SourceEditorId;

    #[test]
    fn handoff_release_lets_the_allocation_through_while_authority_is_held() {
        let mut sync = SourceSyncState::default();
        sync.acquire_shell_focus();
        assert!(sync.shell_focus_held());

        release_for_handoff(&mut sync);

        assert!(!sync.shell_focus_held());
        assert!(
            sync.allocate_focus_interaction(SourceEditorId::Text, "handoff".into())
                .is_some()
        );
    }

    #[test]
    fn without_the_handoff_release_the_allocation_is_still_refused() {
        // Items that are not handoff activations keep today's fallback: a
        // refused allocation under held authority (RFC-042 §6.2 rule 2).
        let mut sync = SourceSyncState::default();
        sync.acquire_shell_focus();

        assert_eq!(
            sync.allocate_focus_interaction(SourceEditorId::Text, "surface item".into()),
            None
        );
        assert!(sync.shell_focus_held());
    }

    #[test]
    fn the_helper_releases_before_it_submits_and_never_restores() {
        let source = include_str!("handoff.rs");
        let body = source
            .split("pub fn submit_handoff_activation(")
            .nth(1)
            .and_then(|rest| rest.split("#[cfg(test)]").next())
            .expect("helper body");
        let release = body.find("release_for_handoff(").expect("releases");
        let submit = body.find("submit_source_interaction(").expect("submits");
        assert!(release < submit, "authority is released before allocation");
        assert!(!body.contains("focus_element"), "a handoff never restores");
        assert!(!body.contains("acquire_shell_focus"));
    }

    #[test]
    fn search_result_new_file_and_split_all_activate_through_the_helper() {
        // §5.4: Split has no runtime phase; this pins it, and the other two,
        // to the one helper and to closing their surface in the finalizer.
        let search = include_str!("../components/search_panel.rs");
        let app_bar = include_str!("../components/app_bar.rs");
        let header = include_str!("../components/editor_header.rs");
        for (name, source, origin, close) in [
            (
                "search_panel",
                search,
                "SourceInteractionOrigin::search_result(",
                "search_open.set(false)",
            ),
            (
                "app_bar",
                app_bar,
                "SourceInteractionOrigin::removable_menu_control(\"appbar-new\")",
                "open_menu.set(OpenMenu::None)",
            ),
            (
                "editor_header",
                header,
                "SourceInteractionOrigin::removable_menu_control(\"mode-split\")",
                "open_menu.set(OpenMenu::None)",
            ),
        ] {
            let call = source
                .split("submit_handoff_activation(")
                .nth(1)
                .unwrap_or_else(|| panic!("{name} does not use the handoff helper"));
            let call = &call[..call.find(");").expect("call end")];
            assert!(call.contains(origin), "{name} origin");
            assert!(call.contains(close), "{name} closes in the finalizer");
            assert!(!source.contains("submit_source_interaction("), "{name}");
        }

        // Explicit dismissal is unchanged: Escape and close still restore.
        assert!(
            search.contains("shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH)")
        );
        assert!(app_bar.contains("release_and_restore_menu_focus(source_sync, OpenMenu::App)"));
        assert!(header.contains("shell_focus::focus_element(shell_focus::TRIGGER_EDITOR_TOOLS)"));
    }
}
