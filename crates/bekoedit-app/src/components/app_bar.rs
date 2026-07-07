//! Persistent application bar (top of every screen).
//!
//! Provides:
//! - bekoedit logo — clicking clears the workspace (returns to start screen)
//! - File menu — Open Folder…, New File, ─, Close Workspace
//! - Language toggle (EN ↔ JA)
//! - Settings gear

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn AppBar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut lang_sig = use_context::<Signal<Lang>>();
    let lang = *lang_sig.read();
    let mut settings_open = use_context::<Signal<bool>>();
    let mut file_menu_open = use_signal(|| false);

    let has_workspace = state.read().workspace.is_some();

    rsx! {
        header { class: "app-bar",
            // ── Logo / Home ───────────────────────────────────────────────
            button {
                class: "app-bar-logo",
                title: tr(lang, "start.open_folder"),
                onclick: move |_| {
                    // Return to start screen by clearing the workspace
                    state.write().close_workspace();
                },
                "bekoedit"
            }

            // ── File menu ─────────────────────────────────────────────────
            div { class: "app-bar-menu-wrap",
                button {
                    class: if *file_menu_open.read() { "app-bar-btn active" } else { "app-bar-btn" },
                    aria_haspopup: "menu",
                    aria_expanded: "{*file_menu_open.read()}",
                    onclick: move |_| {
                        let cur = *file_menu_open.read();
                        file_menu_open.set(!cur);
                    },
                    {tr(lang, "menu.file")}
                    " ▾"
                }
                if *file_menu_open.read() {
                    div {
                        class: "app-bar-dropdown",
                        role: "menu",
                        // Dismiss on blur
                        onblur: move |_| file_menu_open.set(false),

                        // Open Folder
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                file_menu_open.set(false);
                                let mut st = state;
                                spawn(async move {
                                    if let Some(handle) = rfd::AsyncFileDialog::new()
                                        .set_title("Select workspace folder")
                                        .pick_folder()
                                        .await
                                    {
                                        let _ = st.write().open_workspace(handle.path(), now_ms());
                                    }
                                });
                            },
                            "📂 " {tr(lang, "start.open_folder")}
                        }

                        // New File
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            onclick: move |_| {
                                file_menu_open.set(false);
                                state.write().new_untitled();
                            },
                            "📝 " {tr(lang, "start.new_file")}
                        }

                        // Close Workspace
                        if has_workspace {
                            hr { class: "dropdown-sep" }
                            button {
                                class: "dropdown-item",
                                role: "menuitem",
                                onclick: move |_| {
                                    file_menu_open.set(false);
                                    state.write().close_workspace();
                                },
                                {tr(lang, "menu.close_workspace")}
                            }
                        }
                    }
                }
            }

            // Spacer
            div { class: "app-bar-spacer" }

            // ── Language toggle ───────────────────────────────────────────
            button {
                class: "app-bar-btn",
                title: tr(lang, "lang.switch"),
                onclick: move |_| {
                    let next = if lang == Lang::En { Lang::Ja } else { Lang::En };
                    lang_sig.set(next);
                },
                {tr(lang, "lang.switch")}
            }

            // ── Settings ─────────────────────────────────────────────────
            button {
                class: if *settings_open.read() { "app-bar-btn active" } else { "app-bar-btn" },
                aria_label: tr(lang, "settings.title"),
                onclick: move |_| {
                    let cur = *settings_open.read();
                    settings_open.set(!cur);
                },
                "⚙"
            }
        }
    }
}
