//! Persistent application bar — one logo, one overflow menu.
//!
//! Keeps two items visible at all times: the home logo and a single
//! overflow "⋯" that surfaces everything else. First-time users are not
//! confronted with File menus, language selectors, or settings gears.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::icons::{FolderIcon, NewFileIcon};
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus;
use crate::source_sync::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, cancel_source_focus,
    submit_source_command, submit_source_interaction,
};
use crate::state::{NewFileOpen, OpenMenu, OpenMenuState, SearchOpen};

/// Releases shell focus authority for whichever menu was open, without
/// moving DOM focus. No-ops if no menu was open. For *implicit* dismissal —
/// the user directed attention elsewhere, they did not ask to close the
/// menu — where restoring focus to the trigger would fight whatever focus
/// move caused the dismissal (RFC-042 §7.2 amendment; slice 1 re-review C3).
/// Shared with `app.rs`'s outside-click/focus-leave handler, which cannot
/// otherwise tell which trigger owned the authority being released.
pub(crate) fn release_menu_focus(mut sync: Signal<SourceSyncState>, menu: OpenMenu) {
    if menu == OpenMenu::None {
        return;
    }
    sync.write().release_shell_focus();
}

/// Releases shell focus authority and restores DOM focus to whichever
/// trigger owns `menu`, or no-ops if no menu was open (RFC-042 §6.2 rule 3).
/// Only for *explicit* dismissal of the menu itself — Escape, the trigger's
/// own toggle-close, or activating an item inside it — never for focus
/// simply moving elsewhere (use `release_menu_focus` for that).
pub(crate) fn release_and_restore_menu_focus(mut sync: Signal<SourceSyncState>, menu: OpenMenu) {
    let trigger = match menu {
        OpenMenu::App => shell_focus::TRIGGER_APP_MENU,
        OpenMenu::EditorTools => shell_focus::TRIGGER_EDITOR_TOOLS,
        OpenMenu::None => return,
    };
    sync.write().release_shell_focus();
    shell_focus::focus_element(trigger);
}

#[component]
pub fn AppBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();
    let mut open_menu = use_context::<OpenMenuState>().0;
    let mut search_open = use_context::<SearchOpen>().0;
    let mut new_file_open = use_context::<NewFileOpen>().0;

    let has_workspace = state.read().workspace.is_some();
    let menu_open = *open_menu.read() == OpenMenu::App;

    // Closes the app menu: releases shell authority and restores focus to
    // its trigger. Used by every close path — Escape, item selection, and
    // the trigger's own toggle-close (RFC-042 §6.2 rule 3, handoff §7.4).
    let mut close_app_menu = move || {
        release_and_restore_menu_focus(source_sync, OpenMenu::App);
        open_menu.set(OpenMenu::None);
    };

    rsx! {
        header { class: "app-bar",
            // ── Logo / Home ───────────────────────────────────────────────
            button {
                class: "app-bar-logo",
                title: tr(ui_lang, "app.title"),
                onclick: move |_| {
                    // Navigating home is not "dismissing the menu" as its
                    // primary intent — release only, do not restore focus
                    // to a trigger the user did not ask to return to.
                    release_menu_focus(source_sync, *open_menu.read());
                    open_menu.set(OpenMenu::None);
                    submit_source_command(
                        source_sync,
                        state,
                        mode_sig,
                        toasts,
                        SourceCommand::CloseWorkspace,
                    );
                },
                "bekoedit"
            }

            div { class: "app-bar-spacer" }

            // ── Single overflow menu ─────────────────────────────────────
            div {
                class: "app-bar-menu-wrap",
                onclick: move |event| event.stop_propagation(),
                onfocusin: move |event| event.stop_propagation(),
                onkeydown: move |event| {
                    if event.key() == Key::Escape {
                        close_app_menu();
                    }
                },
                button {
                    id: shell_focus::TRIGGER_APP_MENU,
                    class: if menu_open { "app-bar-btn active" } else { "app-bar-btn" },
                    aria_label: tr(ui_lang, "menu.app"),
                    aria_haspopup: "menu",
                    aria_expanded: "{menu_open}",
                    aria_controls: "app-overflow-menu",
                    title: tr(ui_lang, "menu.app"),
                    onclick: move |_| {
                        if *open_menu.read() == OpenMenu::App {
                            close_app_menu();
                        } else {
                            cancel_source_focus(source_sync);
                            // RFC-042 M4/K1: shell surfaces are mutually
                            // exclusive — opening this menu closes the
                            // workspace-search and new-file disclosures too.
                            search_open.set(false);
                            new_file_open.set(false);
                            open_menu.set(OpenMenu::App);
                        }
                    },
                    "⋯"
                }
                if menu_open {
                    div {
                        id: "app-overflow-menu",
                        class: "app-bar-dropdown",
                        role: "menu",
                        tabindex: "-1",

                        // Open Folder
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                close_app_menu();
                                let st = state;
                                let sync = source_sync;
                                let mode = mode_sig;
                                let toast_sig = toasts;
                                spawn(async move {
                                    if let Some(h) = rfd::AsyncFileDialog::new()
                                        .set_title(tr(ui_lang, "start.open_folder"))
                                        .pick_folder().await
                                    {
                                        submit_source_command(
                                            sync,
                                            st,
                                            mode,
                                            toast_sig,
                                            SourceCommand::OpenWorkspace(h.path().to_path_buf()),
                                        );
                                    }
                                });
                            },
                            FolderIcon {} {tr(ui_lang, "start.open_folder")}
                        }

                        // New File
                        button {
                            class: "dropdown-item",
                            "data-source-focus-launch": "appbar-new",
                            role: "menuitem",
                            onclick: move |_| {
                                crate::bridge::trace("app_bar.new_file.click", "");
                                submit_source_interaction(
                                    source_sync,
                                    state,
                                    mode_sig,
                                    toasts,
                                    SourceCommand::NewUntitled,
                                    SourceInteractionOrigin::removable_menu_control("appbar-new"),
                                    move || open_menu.set(OpenMenu::None),
                                );
                            },
                            NewFileIcon {} {tr(ui_lang, "start.new_file")}
                        }

                        // Close Workspace (only when one is open)
                        if has_workspace {
                            hr { class: "dropdown-sep" }
                            button {
                                class: "dropdown-item",
                                role: "menuitem",
                                onclick: move |_| {
                                    close_app_menu();
                                    submit_source_command(
                                        source_sync,
                                        state,
                                        mode_sig,
                                        toasts,
                                        SourceCommand::CloseWorkspace,
                                    );
                                },
                                {tr(ui_lang, "menu.close_workspace")}
                            }
                        }

                        hr { class: "dropdown-sep" }

                        // Settings
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                crate::bridge::trace("app_bar.settings.click", "");
                                // Settings is a screen replacement (RFC-042
                                // §7.5): it keeps holding shell authority
                                // across this menu closing, so only dismiss
                                // the dropdown here — do not release.
                                cancel_source_focus(source_sync);
                                open_menu.set(OpenMenu::None);
                                submit_source_command(
                                    source_sync,
                                    state,
                                    mode_sig,
                                    toasts,
                                    SourceCommand::OpenSettings,
                                );
                            },
                            "⚙  " {tr(ui_lang, "settings.title")}
                        }
                    }
                }
            }
        }
    }
}
