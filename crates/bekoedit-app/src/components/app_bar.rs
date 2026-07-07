//! Persistent application bar — one logo, one overflow menu.
//!
//! Keeps two items visible at all times: the home logo and a single
//! overflow "⋯" that surfaces everything else. First-time users are not
//! confronted with File menus, language selectors, or settings gears.

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn AppBar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut lang_sig = use_context::<Signal<Lang>>();
    let ui_lang = *lang_sig.read();
    let mut settings_open = use_context::<Signal<bool>>();
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
                    state.write().close_workspace();
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
                                let mut st = state;
                                spawn(async move {
                                    if let Some(h) = rfd::AsyncFileDialog::new()
                                        .set_title("Select workspace folder")
                                        .pick_folder().await
                                    {
                                        let _ = st.write().open_workspace(h.path(), now_ms());
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
                                menu_open.set(false);
                                state.write().new_untitled();
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
                                    state.write().close_workspace();
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
                                menu_open.set(false);
                                settings_open.set(true);
                            },
                            "⚙  " {tr(ui_lang, "settings.title")}
                        }
                    }
                }
            }
        }
    }
}
