//! Start screen (external design §18): open a folder as workspace, or
//! pick a recent workspace. Open failures surface inline without crashing
//! (RFC-003 acceptance).

use std::path::PathBuf;

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};
use crate::state::now_secs;

#[component]
pub fn StartScreen() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut path_input = use_signal(String::new);
    let mut error = use_signal(String::new);

    let mut open = move |path: PathBuf| {
        let result = state.write().open_workspace(&path, now_secs());
        if let Err(e) = result {
            error.set(e.to_string());
        }
    };

    let recents = state.read().recents.entries.clone();

    rsx! {
        div { class: "start-screen",
            h1 { {tr(lang, "app.title")} }
            p { class: "tagline", {tr(lang, "start.tagline")} }
            div { class: "open-row",
                input {
                    r#type: "text",
                    placeholder: tr(lang, "start.path_placeholder"),
                    value: "{path_input}",
                    oninput: move |evt| path_input.set(evt.value()),
                }
                button {
                    class: "primary",
                    onclick: move |_| open(PathBuf::from(path_input.read().trim())),
                    {tr(lang, "start.open")}
                }
            }
            if !error.read().is_empty() {
                p { class: "error", "{error}" }
            }
            h2 { {tr(lang, "start.recents")} }
            if recents.is_empty() {
                p { class: "muted", {tr(lang, "start.no_recents")} }
            } else {
                ul { class: "recents",
                    for entry in recents {
                        li {
                            button {
                                onclick: {
                                    let path = entry.root_path.clone();
                                    move |_| open(path.clone())
                                },
                                span { class: "recent-name", "{entry.display_name}" }
                                span { class: "recent-path", "{entry.root_path.display()}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
