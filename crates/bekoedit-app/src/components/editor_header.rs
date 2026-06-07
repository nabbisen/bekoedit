//! Editor header (RFC-010/019/020): mode switch, save, settings toggle,
//! explorer collapse (RFC-020 Ctrl+B), language switcher.
//! ARIA: mode buttons use role="tab" semantics via aria-pressed (RFC-021).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn EditorHeader() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut lang_signal = use_context::<Signal<Lang>>();
    let lang = *lang_signal.read();
    let mut mode_signal = use_context::<Signal<EditorMode>>();
    let mode = *mode_signal.read();
    let mut collapsed = use_context::<Signal<bool>>();
    let mut settings_open = use_context::<Signal<bool>>();
    let mut toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();

    let doc_name = state
        .read()
        .session
        .as_ref()
        .and_then(|s| s.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let dirty = state.read().session.as_ref().is_some_and(|s| s.dirty);
    let has_doc = state.read().session.is_some();

    rsx! {
        header {
            class: "editor-header",
            role: "toolbar",
            aria_label: tr(lang, "editor.toolbar_label"),

            // Explorer collapse toggle (RFC-020: Ctrl+B / button).
            button {
                class: "icon-btn",
                aria_label: tr(lang, "explorer.toggle"),
                aria_pressed: "{*collapsed.read()}",
                onclick: move |_| {
                    let c = *collapsed.read();
                    collapsed.set(!c);
                },
                "☰"
            }

            span {
                class: "doc-name",
                aria_live: "polite",
                "{doc_name}"
                if dirty {
                    span { class: "dirty-dot", aria_label: tr(lang, "save.dirty"), "●" }
                }
            }

            // Mode switch (RFC-019): visually a tab strip.
            nav {
                class: "mode-switch",
                role: "tablist",
                aria_label: tr(lang, "editor.mode_label"),
                for (target, key) in [
                    (EditorMode::Text,    "mode.text"),
                    (EditorMode::Form,    "mode.form"),
                    (EditorMode::Preview, "mode.preview"),
                ] {
                    button {
                        role: "tab",
                        class: if mode == target { "mode active" } else { "mode" },
                        aria_selected: "{mode == target}",
                        onclick: move |_| mode_signal.set(target),
                        {tr(lang, key)}
                    }
                }
            }

            div { class: "header-actions",
                if has_doc {
                    button {
                        class: "primary",
                        aria_label: tr(lang, "editor.save"),
                        onclick: move |_| {
                            if let Err(e) = state.write().save_now(now_ms()) {
                                push_toast(&mut toasts, ToastKind::Error, e.to_string());
                            } else {
                                push_toast(&mut toasts, ToastKind::Success, tr(lang, "save.saved"));
                            }
                        },
                        {tr(lang, "editor.save")}
                    }
                }
                button {
                    aria_label: tr(lang, "settings.title"),
                    onclick: move |_| settings_open.set(true),
                    "⚙"
                }
                button {
                    aria_label: tr(lang, "lang.switch"),
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
