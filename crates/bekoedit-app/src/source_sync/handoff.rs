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
//! It is a handoff **only when the action claims focus in the current mode**.
//! An `OpenDocument` in Preview or Form -- Form is the default mode -- claims
//! nothing, so it is not one: the surface's explicit close runs (release
//! **and** restore), then the command is submitted.
//!
//! Every other close path -- Escape, the trigger's toggle, items whose action
//! is not a source-focus interaction -- keeps releasing and restoring.

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;
use dioxus::prelude::*;

use crate::components::toast::Toast;

use super::focus::claims_focus;
use super::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, submit_source_command,
    submit_source_interaction,
};

/// How activating an item inside a shell surface closes that surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dismissal {
    /// The action claims editor focus: release, never restore.
    Handoff,
    /// The action claims nothing: release and restore to the trigger.
    Explicit,
}

fn dismissal_for(
    command: &SourceCommand,
    current_mode: EditorMode,
    sync: &SourceSyncState,
) -> Dismissal {
    if claims_focus(command, current_mode, sync) {
        Dismissal::Handoff
    } else {
        Dismissal::Explicit
    }
}

/// Step 1, kept apart so the ordering is testable without a WebView.
fn release_for_handoff(sync: &mut SourceSyncState) {
    sync.release_shell_focus();
}

/// Activates an item that may hand focus off. With a claim, `close_surface`
/// closes the shell surface's DOM and runs as the launch finalizer, never
/// before the guard has armed. Without one, `explicit_close` -- the surface's
/// own release-and-restore close -- runs before the command is submitted.
#[allow(clippy::too_many_arguments)]
pub fn submit_handoff_activation(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    close_surface: impl FnOnce() + 'static,
    explicit_close: impl FnOnce(),
) {
    let current_mode = *mode.read();
    let dismissal = dismissal_for(&command, current_mode, &sync.read());
    match dismissal {
        Dismissal::Handoff => {
            release_for_handoff(&mut sync.write());
            submit_source_interaction(sync, state, mode, toasts, command, origin, close_surface);
        }
        Dismissal::Explicit => {
            explicit_close();
            submit_source_command(sync, state, mode, toasts, command);
        }
    }
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

    const ALL_MODES: [EditorMode; 4] = [
        EditorMode::Text,
        EditorMode::Split,
        EditorMode::Preview,
        EditorMode::Form,
    ];

    /// A controller with `editor_id` mounted and nothing pending -- the
    /// ordinary case these tests encode, where the controller and the UI mode
    /// signal agree. Task 022's own tests cover the case where they do not.
    fn ready_editor(editor_id: SourceEditorId) -> crate::source_sync::lifecycle::ReadyEditor {
        use bekoedit_ui_contract::source_editor::{EditorIdentity, EditorInstanceId, SourceEpoch};
        crate::source_sync::lifecycle::ReadyEditor {
            identity: EditorIdentity {
                instance_id: EditorInstanceId::new(1),
                editor_id,
                document_id: 1,
                epoch: SourceEpoch::new(1),
            },
            revision: 1,
            last_seq: 0,
        }
    }

    /// A controller whose mounted editor matches `mode`, or nothing mounted
    /// for Preview and Form, which are not source editors.
    fn synced_to(mode: EditorMode) -> SourceSyncState {
        let mut sync = SourceSyncState::default();
        let editor_id = match mode {
            EditorMode::Text => Some(SourceEditorId::Text),
            EditorMode::Split => Some(SourceEditorId::Split),
            EditorMode::Preview | EditorMode::Form => None,
        };
        if let Some(editor_id) = editor_id {
            sync.lifecycle.state =
                crate::source_sync::lifecycle::LifecycleState::Ready(ready_editor(editor_id));
        }
        sync
    }

    #[test]
    fn open_document_is_a_handoff_only_in_the_modes_that_claim_focus() {
        // Re-review §2: Form is the default mode, and there a search result
        // claims nothing, so it must restore rather than hand off.
        let open = SourceCommand::OpenDocument("sub/child.md".into());
        assert_eq!(
            dismissal_for(&open, EditorMode::Text, &synced_to(EditorMode::Text)),
            Dismissal::Handoff
        );
        assert_eq!(
            dismissal_for(&open, EditorMode::Split, &synced_to(EditorMode::Split)),
            Dismissal::Handoff
        );
        assert_eq!(
            dismissal_for(&open, EditorMode::Preview, &synced_to(EditorMode::Preview)),
            Dismissal::Explicit
        );
        assert_eq!(
            dismissal_for(&open, EditorMode::Form, &synced_to(EditorMode::Form)),
            Dismissal::Explicit
        );
    }

    #[test]
    fn new_file_is_a_handoff_from_every_mode() {
        // Its no-claim path is unreachable today; the helper stays general.
        for current in ALL_MODES {
            assert_eq!(
                dismissal_for(&SourceCommand::NewUntitled, current, &synced_to(current)),
                Dismissal::Handoff,
                "from {current:?}"
            );
        }
    }

    #[test]
    fn switch_mode_into_the_mode_already_current_is_not_a_handoff() {
        // Re-review §3: `submit_interaction` also returns early on
        // `is_same_source_mode`, so no claim is made. Releasing without
        // restoring there would strand focus on the body, as in §2.
        assert_eq!(
            dismissal_for(
                &SourceCommand::SwitchMode(EditorMode::Text),
                EditorMode::Text,
                &synced_to(EditorMode::Text)
            ),
            Dismissal::Explicit
        );
        assert_eq!(
            dismissal_for(
                &SourceCommand::SwitchMode(EditorMode::Split),
                EditorMode::Split,
                &synced_to(EditorMode::Split)
            ),
            Dismissal::Explicit
        );

        // Switching to a different source mode still hands off.
        assert_eq!(
            dismissal_for(
                &SourceCommand::SwitchMode(EditorMode::Split),
                EditorMode::Text,
                &synced_to(EditorMode::Text)
            ),
            Dismissal::Handoff
        );
        assert_eq!(
            dismissal_for(
                &SourceCommand::SwitchMode(EditorMode::Text),
                EditorMode::Split,
                &synced_to(EditorMode::Split)
            ),
            Dismissal::Handoff
        );
        // The editor-tools item toggles, so it never submits its own mode.
        for current in ALL_MODES {
            let target = if current == EditorMode::Split {
                EditorMode::Text
            } else {
                EditorMode::Split
            };
            assert_eq!(
                dismissal_for(
                    &SourceCommand::SwitchMode(target),
                    current,
                    &synced_to(current)
                ),
                Dismissal::Handoff,
                "split item from {current:?}"
            );
        }
    }

    #[test]
    fn a_switch_in_flight_to_a_different_target_still_hands_off_regardless_of_the_ui_mode() {
        // Task 022 case A, reached through the Split menu item: the UI mode
        // signal still says Text while a switch to Preview is in flight, but
        // the controller is heading to Preview, so this Split activation must
        // still claim focus rather than being read as already-current.
        let mut sync = SourceSyncState::default();
        sync.lifecycle.state = crate::source_sync::lifecycle::LifecycleState::SnapshotPending {
            editor: ready_editor(SourceEditorId::Text),
            command: SourceCommand::SwitchMode(EditorMode::Preview),
            operation: crate::source_sync::lifecycle::PendingOperation {
                operation_id: bekoedit_ui_contract::source_editor::OperationId::new(9),
                deadline_ms: u64::MAX,
            },
        };
        assert_eq!(
            dismissal_for(
                &SourceCommand::SwitchMode(EditorMode::Split),
                EditorMode::Text,
                &sync
            ),
            Dismissal::Handoff
        );
    }

    #[test]
    fn the_helper_restores_only_on_the_no_claim_path() {
        let source = include_str!("handoff.rs");
        let body = source
            .split("pub fn submit_handoff_activation(")
            .nth(1)
            .and_then(|rest| rest.split("#[cfg(test)]").next())
            .expect("helper body");
        let explicit_at = body.find("Dismissal::Explicit =>").expect("explicit arm");
        let handoff = &body[body.find("Dismissal::Handoff =>").expect("handoff arm")..explicit_at];
        let explicit = &body[explicit_at..];

        let release = handoff
            .find("release_for_handoff(")
            .expect("handoff releases");
        let submit = handoff
            .find("submit_source_interaction(")
            .expect("handoff claims");
        assert!(release < submit, "authority is released before allocation");
        assert!(
            !handoff.contains("explicit_close"),
            "a handoff never restores"
        );

        let close = explicit
            .find("explicit_close()")
            .expect("no-claim path restores");
        let plain = explicit
            .find("submit_source_command(")
            .expect("no-claim path submits");
        assert!(close < plain, "the explicit close runs before the command");
        assert!(!explicit.contains("release_for_handoff"));
        assert!(!body.contains("acquire_shell_focus"));
    }

    #[test]
    fn search_result_new_file_and_split_all_activate_through_the_helper() {
        // §5.4: Split has no runtime phase; this pins it, and the other two,
        // to the one helper and to closing their surface in the finalizer.
        let search = include_str!("../components/search_panel.rs");
        let app_bar = include_str!("../components/app_bar.rs");
        let header = include_str!("../components/editor_header.rs");
        for (name, source, origin, close, explicit) in [
            (
                "search_panel",
                search,
                "SourceInteractionOrigin::search_result(",
                "move || search_open.set(false),",
                "close_search,",
            ),
            (
                "app_bar",
                app_bar,
                "SourceInteractionOrigin::removable_menu_control(\"appbar-new\")",
                "move || open_menu.set(OpenMenu::None),",
                "close_app_menu,",
            ),
            (
                "editor_header",
                header,
                "SourceInteractionOrigin::removable_menu_control(\"mode-split\")",
                "move || open_menu.set(OpenMenu::None),",
                "close_editor_tools_menu,",
            ),
        ] {
            let call = source
                .split("submit_handoff_activation(")
                .nth(1)
                .unwrap_or_else(|| panic!("{name} does not use the handoff helper"));
            // The call ends at its own closing line, however it is indented.
            let end = call
                .lines()
                .take_while(|line| line.trim() != ");")
                .map(|line| line.len() + 1)
                .sum::<usize>();
            let call = &call[..end];
            assert!(call.contains(origin), "{name} origin");
            assert!(call.contains(close), "{name} closes in the finalizer");
            assert!(call.contains(explicit), "{name} passes its explicit close");
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
