//! Persistent application bar — one logo, one overflow menu.
//!
//! Keeps two items visible at all times: the home logo and a single
//! overflow "⋯" that surfaces everything else. First-time users are not
//! confronted with File menus, language selectors, or settings gears.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::source_sync::{SourceCommand, SourceSyncState, submit_source_command};

#[component]
pub fn AppBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let mut lang_sig = use_context::<Signal<Lang>>();
    let ui_lang = *lang_sig.read();
    let mut menu_open = use_signal(|| false);

    let has_workspace = state.read().workspace.is_some();

    rsx! {
        header { class: "app-bar",
            // ── Logo / Home ───────────────────────────────────────────────
            button {
                class: "app-bar-logo",
                title: tr(ui_lang, "app.title"),
                onclick: move |_| {
                    let o = *menu_open.read();
                    if o { menu_open.set(false); }
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
            div { class: "app-bar-menu-wrap",
                button {
                    class: if *menu_open.read() { "app-bar-btn active" } else { "app-bar-btn" },
                    aria_label: "Menu",
                    aria_haspopup: "menu",
                    aria_expanded: "{*menu_open.read()}",
                    onclick: move |_| {
                        let cur = *menu_open.read();
                        menu_open.set(!cur);
                    },
                    "⋯"
                }
                if *menu_open.read() {
                    div {
                        class: "app-bar-dropdown",
                        role: "menu",
                        tabindex: "-1",

                        // Open Folder
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                menu_open.set(false);
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
                            "📂  " {tr(ui_lang, "start.open_folder")}
                        }

                        // New File
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                crate::bridge::trace("app_bar.new_file.click", "");
                                menu_open.set(false);
                                submit_source_command(
                                    source_sync,
                                    state,
                                    mode_sig,
                                    toasts,
                                    SourceCommand::NewUntitled,
                                );
                            },
                            "📝  " {tr(ui_lang, "start.new_file")}
                        }

                        // Close Workspace (only when one is open)
                        if has_workspace {
                            hr { class: "dropdown-sep" }
                            button {
                                class: "dropdown-item",
                                role: "menuitem",
                                onclick: move |_| {
                                    menu_open.set(false);
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

                        // Language
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                menu_open.set(false);
                                lang_sig.set(if ui_lang == Lang::En { Lang::Ja } else { Lang::En });
                            },
                            "🌐  " {tr(ui_lang, "lang.switch")}
                        }

                        // Settings
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                crate::bridge::trace("app_bar.settings.click", "");
                                menu_open.set(false);
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
