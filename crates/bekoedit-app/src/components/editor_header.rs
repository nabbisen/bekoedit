//! Editor header (RFC-010): active document name, the Text/Form/Preview
//! mode switch (RFC-019; Split Mode deferred post-MVP per the 2026-06-07
//! review resolution), manual save, and the language toggle.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn EditorHeader() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut lang_signal = use_context::<Signal<Lang>>();
    let lang = *lang_signal.read();
    let mut mode_signal = use_context::<Signal<EditorMode>>();
    let mode = *mode_signal.read();

    let doc_name = state
        .read()
        .session
        .as_ref()
        .and_then(|s| s.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let dirty = state.read().session.as_ref().is_some_and(|s| s.dirty);
    let has_doc = state.read().session.is_some();

    let mode_button = |target: EditorMode, key: &'static str| {
        let active = mode == target;
        rsx! {
            button {
                class: if active { "mode active" } else { "mode" },
                onclick: move |_| mode_signal.set(target),
                {tr(lang, key)}
            }
        }
    };

    rsx! {
        header { class: "editor-header",
            span { class: "doc-name",
                "{doc_name}"
                if dirty {
                    span { class: "dirty-dot", "●" }
                }
            }
            nav { class: "mode-switch",
                {mode_button(EditorMode::Text, "mode.text")}
                {mode_button(EditorMode::Form, "mode.form")}
                {mode_button(EditorMode::Preview, "mode.preview")}
            }
            div { class: "header-actions",
                if has_doc {
                    button {
                        class: "primary",
                        onclick: move |_| {
                            let _ = state.write().save_now(now_ms());
                        },
                        {tr(lang, "editor.save")}
                    }
                }
                button {
                    onclick: move |_| {
                        let next = lang.toggle();
                        lang_signal.set(next);
                    },
                    {tr(lang, "lang.switch")}
                }
            }
        }
    }
}
