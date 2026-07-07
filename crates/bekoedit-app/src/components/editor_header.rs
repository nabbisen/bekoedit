//! Editor header: mode switch, outline, search, export, save, settings.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn EditorHeader() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut lang_sig = use_context::<Signal<Lang>>();
    let lang = *lang_sig.read();
    let mut mode_sig = use_context::<Signal<EditorMode>>();
    let mode = *mode_sig.read();
    let mut collapsed = use_context::<Signal<bool>>(); // explorer
    let mut outline_open = use_context::<Signal<bool>>(); // 3rd bool
    let mut search_open = use_context::<Signal<bool>>();
    let mut backlinks_open = use_context::<Signal<bool>>();
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

            // Explorer toggle
            button {
                class: "icon-btn",
                aria_label: tr(lang, "explorer.toggle"),
                aria_pressed: "{*collapsed.read()}",
                onclick: move |_| { let c = *collapsed.read(); collapsed.set(!c); },
                "☰"
            }

            // Document name + dirty indicator
            span {
                class: "doc-name", aria_live: "polite",
                "{doc_name}"
                if dirty { span { class: "dirty-dot", aria_label: tr(lang, "save.dirty"), "●" } }
            }

            // Mode switch
            nav { class: "mode-switch", role: "tablist", aria_label: tr(lang, "editor.mode_label"),
                for (target, key) in [
                    (EditorMode::Text,    "mode.text"),
                    (EditorMode::Form,    "mode.form"),
                    (EditorMode::Preview, "mode.preview"),
                    (EditorMode::Split,   "mode.split"),
                ] {
                    button {
                        role: "tab",
                        class: if mode == target { "mode active" } else { "mode" },
                        aria_selected: "{mode == target}",
                        onclick: move |_| mode_sig.set(target),
                        {tr(lang, key)}
                    }
                }
            }

            div { class: "header-actions",
                // Search toggle (RFC-033)
                button {
                    class: if *search_open.read() { "icon-btn active" } else { "icon-btn" },
                    aria_label: tr(lang, "search.toggle"),
                    aria_pressed: "{*search_open.read()}",
                    onclick: move |_| {
                        let o = *search_open.read();
                        search_open.set(!o);
                        if !o { outline_open.set(false); backlinks_open.set(false); }
                    },
                    "🔍"
                }
                // Backlinks toggle (RFC-034)
                if has_doc {
                    button {
                        class: if *backlinks_open.read() { "icon-btn active" } else { "icon-btn" },
                        aria_label: tr(lang, "backlinks.title"),
                        aria_pressed: "{*backlinks_open.read()}",
                        onclick: move |_| {
                            let o = *backlinks_open.read();
                            backlinks_open.set(!o);
                            if !o { search_open.set(false); outline_open.set(false); }
                        },
                        "⬡"
                    }
                }
                // Outline toggle (RFC-010)
                if has_doc {
                    button {
                        class: if *outline_open.read() { "icon-btn active" } else { "icon-btn" },
                        aria_label: tr(lang, "outline.toggle"),
                        aria_pressed: "{*outline_open.read()}",
                        onclick: move |_| {
                            let o = *outline_open.read();
                            outline_open.set(!o);
                            if !o { search_open.set(false); backlinks_open.set(false); }
                        },
                        "≡"
                    }
                }
                // Export to HTML (RFC-035)
                if has_doc {
                    button {
                        aria_label: tr(lang, "export.html"),
                        onclick: move |_| {
                            if let Some(sess) = state.read().session.as_ref() {
                                let export_path = sess.path.with_extension("html");
                                match state.read().export_html(&export_path) {
                                    Ok(()) => push_toast(&mut toasts, ToastKind::Success,
                                        format!("{} → {}", tr(lang, "export.html"),
                                                export_path.file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or_default())),
                                    Err(e) => push_toast(&mut toasts, ToastKind::Error, e.to_string()),
                                }
                            }
                        },
                        {tr(lang, "export.html")}
                    }
                }
                // Save
                if has_doc {
                    button {
                        class: "primary", aria_label: tr(lang, "editor.save"),
                        onclick: move |_| {
                            match state.write().save_now(now_ms()) {
                                Ok(())  => push_toast(&mut toasts, ToastKind::Success, tr(lang, "save.saved")),
                                Err(e)  => push_toast(&mut toasts, ToastKind::Error, e.to_string()),
                            }
                        },
                        {tr(lang, "editor.save")}
                    }
                }
                // Settings
                button {
                    aria_label: tr(lang, "settings.title"),
                    onclick: move |_| settings_open.set(true),
                    "⚙"
                }
                // Language switch
                button {
                    aria_label: tr(lang, "lang.switch"),
                    onclick: move |_| lang_sig.set(lang.toggle()),
                    {tr(lang, "lang.switch")}
                }
            }
        }
    }
}
