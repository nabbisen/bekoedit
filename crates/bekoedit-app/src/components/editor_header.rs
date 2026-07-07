//! Editor header — minimal primary bar, advanced features behind "•••".
//!
//! Tier 1 (always visible):  filename · save state · Text / Preview tabs · Save
//! Tier 2 (on demand):       Form · Outline
//! Tier 3 (power, via "•••"): Split · Search · Backlinks · History · Export
//!
//! Undo/Redo live on the keyboard (Ctrl+Z / Ctrl+Y). They don't need
//! buttons — surfacing them in the toolbar teaches the wrong habit.

use dioxus::prelude::*;

use bekoedit_core::{AppState, SaveState};
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn EditorHeader() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut mode_sig = use_context::<Signal<EditorMode>>();
    let mode = *mode_sig.read();
    let mut collapsed = use_context::<Signal<bool>>();
    // advanced panel signals (3rd–7th bools)
    let mut outline_open = use_context::<Signal<bool>>();
    let mut search_open = use_context::<Signal<bool>>();
    let mut backlinks_open = use_context::<Signal<bool>>();
    let mut history_open = use_context::<Signal<bool>>();
    let mut toasts = use_context::<Signal<Vec<Toast>>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();

    let session = state.read();
    let doc = session.session.as_ref();
    let file_name = doc
        .and_then(|s| s.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let dirty = doc.is_some_and(|s| s.dirty);
    let is_untitled = doc.is_some_and(|s| s.is_untitled);
    let has_doc = doc.is_some();
    let save_label = match &session.save_state {
        SaveState::Clean => "save.clean",
        SaveState::Dirty => "save.dirty",
        SaveState::Saving => "save.saving",
        SaveState::Saved { .. } => "save.clean",
        SaveState::SaveFailed { .. } => "save.failed",
        SaveState::ConflictResolutionRequired => "save.conflict",
        SaveState::AutoSaveScheduled { .. } => "save.dirty",
    };
    drop(session);

    let mut adv_open = use_signal(|| false);

    rsx! {
        header { class: "editor-header", role: "toolbar", aria_label: tr(ui_lang, "editor.toolbar_label"),
            // ── Explorer toggle (keep as-is, single icon) ─────────────────
            button {
                class: "icon-btn",
                aria_label: tr(ui_lang, "explorer.toggle"),
                onclick: move |_| { let o = *collapsed.read(); collapsed.set(!o); },
                if *collapsed.read() { "›" } else { "‹" }
            }

            // ── Filename ──────────────────────────────────────────────────
            if has_doc {
                span {
                    class: if is_untitled { "file-name untitled" } else { "file-name" },
                    if is_untitled { {tr(ui_lang, "editor.untitled")} } else { "{file_name}" }
                }
                if dirty { span { class: "dirty-dot", aria_label: tr(ui_lang, "save.dirty"), "●" } }
            }

            div { class: "header-spacer" }

            // ── Tier 1: mode tabs (Text + Preview only by default) ────────
            if has_doc {
                nav { class: "mode-switch", role: "tablist", aria_label: tr(ui_lang, "editor.mode_label"),
                    for (m, key) in [
                        (EditorMode::Text,    "mode.text"),
                        (EditorMode::Preview, "mode.preview"),
                    ] {
                        button {
                            class: if mode == m { "mode-tab active" } else { "mode-tab" },
                            role: "tab",
                            aria_selected: "{mode == m}",
                            onclick: move |_| mode_sig.set(m),
                            {tr(ui_lang, key)}
                        }
                    }
                    // Form Mode — still in the primary bar but AFTER Text/Preview
                    button {
                        class: if mode == EditorMode::Form { "mode-tab active" } else { "mode-tab mode-tab-secondary" },
                        role: "tab",
                        aria_selected: "{mode == EditorMode::Form}",
                        onclick: move |_| mode_sig.set(EditorMode::Form),
                        {tr(ui_lang, "mode.form")}
                    }
                }
            }

            div { class: "header-spacer" }

            // ── Tier 1: Save ──────────────────────────────────────────────
            if has_doc {
                span {
                    class: "save-status {save_label.replace('.', \"-\")}",
                    aria_live: "polite",
                    {tr(ui_lang, save_label)}
                }

                if is_untitled {
                    button {
                        class: "btn-secondary save-as-btn",
                        title: tr(ui_lang, "editor.save_as"),
                        onclick: move |_| {
                            let mut st = state;
                            let lv = ui_lang;
                            spawn(async move {
                                if let Some(h) = rfd::AsyncFileDialog::new()
                                    .set_title(tr(lv, "editor.save_as_title"))
                                    .add_filter("Markdown", &["md","markdown"])
                                    .save_file().await
                                {
                                    match st.write().save_as(h.path().to_path_buf(), now_ms()) {
                                        Ok(()) => push_toast(&mut use_context::<Signal<Vec<Toast>>>(), ToastKind::Success, tr(lv, "save.saved")),
                                        Err(e) => push_toast(&mut use_context::<Signal<Vec<Toast>>>(), ToastKind::Error, e.to_string()),
                                    }
                                }
                            });
                        },
                        {tr(ui_lang, "editor.save_as")}
                    }
                } else {
                    button {
                        class: "btn-primary",
                        aria_label: tr(ui_lang, "editor.save"),
                        onclick: move |_| {
                            match state.write().save_now(now_ms()) {
                                Ok(())  => push_toast(&mut toasts, ToastKind::Success, tr(ui_lang, "save.saved")),
                                Err(e)  => push_toast(&mut toasts, ToastKind::Error,   e.to_string()),
                            }
                        },
                        {tr(ui_lang, "editor.save")}
                    }
                }

                // ── Tier 3: "•••" advanced overflow ────────────────────────
                button {
                    class: if *adv_open.read() { "icon-btn active" } else { "icon-btn" },
                    title: "More",
                    aria_label: "More tools",
                    onclick: move |_| { let o = *adv_open.read(); adv_open.set(!o); },
                    "•••"
                }
                if *adv_open.read() {
                    div { class: "adv-dropdown", role: "menu",
                        // Split
                        button {
                            class: if mode == EditorMode::Split { "dropdown-item active" } else { "dropdown-item" },
                            onclick: move |_| { mode_sig.set(EditorMode::Split); adv_open.set(false); },
                            {tr(ui_lang, "mode.split")}
                        }
                        hr { class: "dropdown-sep" }
                        // Outline
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                let o = *outline_open.read();
                                outline_open.set(!o);
                                adv_open.set(false);
                            },
                            {tr(ui_lang, "outline.title")}
                        }
                        // Search
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                let o = *search_open.read();
                                search_open.set(!o);
                                adv_open.set(false);
                            },
                            {tr(ui_lang, "search.label")}
                        }
                        // Backlinks
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                let o = *backlinks_open.read();
                                backlinks_open.set(!o);
                                adv_open.set(false);
                            },
                            {tr(ui_lang, "backlinks.title")}
                        }
                        // History
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                let o = *history_open.read();
                                history_open.set(!o);
                                adv_open.set(false);
                            },
                            {tr(ui_lang, "history.title")}
                        }
                        hr { class: "dropdown-sep" }
                        // Export HTML
                        button {
                            class: "dropdown-item",
                            onclick: move |_| {
                                adv_open.set(false);
                                let st = state.read();
                                if let Some(session) = st.session.as_ref() {
                                    let html = session.preview_html();
                                    let name = session.path.file_stem()
                                        .map(|s| format!("{}.html", s.to_string_lossy()))
                                        .unwrap_or("export.html".into());
                                    drop(st);
                                    spawn(async move {
                                        if let Some(h) = rfd::AsyncFileDialog::new()
                                            .set_file_name(&name)
                                            .add_filter("HTML", &["html"])
                                            .save_file().await
                                        {
                                            let _ = std::fs::write(h.path(),
                                                format!("<!DOCTYPE html><html><body>{html}</body></html>"));
                                        }
                                    });
                                }
                            },
                            {tr(ui_lang, "export.html")}
                        }
                    }
                }
            }
        }
    }
}
