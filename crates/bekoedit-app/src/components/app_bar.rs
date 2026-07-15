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
use crate::source_sync::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, cancel_source_focus,
    submit_source_command, submit_source_interaction,
};
use crate::state::{OpenMenu, OpenMenuState};

#[component]
pub fn AppBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();
    let mut open_menu = use_context::<OpenMenuState>().0;

    let has_workspace = state.read().workspace.is_some();
    let menu_open = *open_menu.read() == OpenMenu::App;

    rsx! {
        header { class: "app-bar",
            // ── Logo / Home ───────────────────────────────────────────────
            button {
                class: "app-bar-logo",
                title: tr(ui_lang, "app.title"),
                onclick: move |_| {
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
                button {
                    class: if menu_open { "app-bar-btn active" } else { "app-bar-btn" },
                    aria_label: "Menu",
                    aria_haspopup: "menu",
                    aria_expanded: "{menu_open}",
                    onclick: move |_| {
                        open_menu.set(if *open_menu.read() == OpenMenu::App {
                            OpenMenu::None
                        } else {
                            OpenMenu::App
                        });
                    },
                    "⋯"
                }
                if menu_open {
                    div {
                        class: "app-bar-dropdown",
                        role: "menu",
                        tabindex: "-1",

                        // Open Folder
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                cancel_source_focus(source_sync);
                                open_menu.set(OpenMenu::None);
                                let st = state;
                                let sync = source_sync;
                                let mode = mode_sig;
                                let toast_sig = toasts;
                                spawn(async move {
                                    if let Some(h) = rfd::AsyncFileDialog::new()
                                        .set_title("Select workspace folder")
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
                                    open_menu.set(OpenMenu::None);
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
