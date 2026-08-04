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

use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus;
use crate::source_sync::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, cancel_pending_source_focus,
    cancel_source_focus, submit_source_command, submit_source_interaction,
};
use crate::state::{
    BacklinksOpen, ExplorerCollapsed, HistoryOpen, NewFileOpen, OpenMenu, OpenMenuState,
    OutlineOpen, SearchOpen,
};

mod mode_tabs;
use mode_tabs::ModeTabs;

#[component]
pub fn EditorHeader() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let mut source_sync = use_context::<Signal<SourceSyncState>>();
    let mode = *mode_sig.read();
    let mut collapsed = use_context::<ExplorerCollapsed>().0;
    let mut outline_open = use_context::<OutlineOpen>().0;
    let mut search_open = use_context::<SearchOpen>().0;
    let mut backlinks_open = use_context::<BacklinksOpen>().0;
    let mut history_open = use_context::<HistoryOpen>().0;
    let mut open_menu = use_context::<OpenMenuState>().0;
    let mut new_file_open = use_context::<NewFileOpen>().0;
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();

    let session = state.read();
    let doc = session.session.as_ref();
    let file_name = doc
        .and_then(|s| s.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let dirty = doc.is_some_and(|s| s.dirty);
    let is_untitled = doc.is_some_and(|s| s.is_untitled);
    let has_doc = doc.is_some();
    let backlinks_available = session
        .workspace
        .as_ref()
        .zip(doc)
        .is_some_and(|(workspace, document)| document.path.starts_with(&workspace.root_path));
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

    let adv_open = *open_menu.read() == OpenMenu::EditorTools;

    // Closes the editor-tools menu: releases shell authority and restores
    // focus to its trigger. Used by every close path (RFC-042 §6.2 rule 3,
    // handoff §7.4).
    let mut close_editor_tools_menu = move || {
        source_sync.write().release_shell_focus();
        shell_focus::focus_element(shell_focus::TRIGGER_EDITOR_TOOLS);
        open_menu.set(OpenMenu::None);
    };

    // Opens the editor-tools menu: acquires shell authority and enforces the
    // RFC-042 M4/K1 mutual-exclusion set. Shared by the trigger's mouse
    // click and its keyboard Enter/Space/Down/Up (handoff §5.2).
    let mut open_editor_tools_menu = move || {
        cancel_source_focus(source_sync);
        search_open.set(false);
        new_file_open.set(false);
        open_menu.set(OpenMenu::EditorTools);
    };

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
                ModeTabs { mode, ui_lang, state, mode_sig, source_sync, toasts }
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
                            // A native OS dialog, not an in-app shell surface —
                            // cancel any pending focus intent but don't claim
                            // shell authority; there is no close path to
                            // release it from (RFC-042 slice 1 re-review, C2).
                            cancel_pending_source_focus(source_sync);
                            let lv = ui_lang;
                            let sync = source_sync;
                            let st = state;
                            let mode = mode_sig;
                            let toast_sig = toasts;
                            spawn(async move {
                                if let Some(h) = rfd::AsyncFileDialog::new()
                                    .set_title(tr(lv, "editor.save_as_title"))
                                    .add_filter("Markdown", &["md","markdown"])
                                    .save_file().await
                                {
                                    submit_source_command(
                                        sync,
                                        st,
                                        mode,
                                        toast_sig,
                                        SourceCommand::SaveAs(h.path().to_path_buf()),
                                    );
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
                            submit_source_command(
                                source_sync,
                                state,
                                mode_sig,
                                toasts,
                                SourceCommand::SaveNow,
                            );
                        },
                        {tr(ui_lang, "editor.save")}
                    }
                }

                // ── Tier 3: "•••" advanced overflow ────────────────────────
                div {
                    class: "adv-menu-wrap",
                    onclick: move |event| event.stop_propagation(),
                    onfocusin: move |event| event.stop_propagation(),
                    onkeydown: move |event| {
                        if event.key() == Key::Escape {
                            close_editor_tools_menu();
                            return;
                        }
                        // Inside-menu navigation only (handoff §5.2). The
                        // trigger's own Down/Up/Enter/Space are handled on
                        // the trigger button and stop propagation before
                        // reaching here.
                        if let Some(target) = shell_focus::menu_item_key_intent(&event.key()) {
                            event.prevent_default();
                            shell_focus::focus_menu_item(shell_focus::MENU_EDITOR_TOOLS, target);
                        }
                    },
                    button {
                        id: shell_focus::TRIGGER_EDITOR_TOOLS,
                        class: if adv_open { "icon-btn active" } else { "icon-btn" },
                        title: tr(ui_lang, "menu.editor_tools"),
                        aria_label: tr(ui_lang, "menu.editor_tools"),
                        aria_haspopup: "menu",
                        aria_expanded: "{adv_open}",
                        aria_controls: shell_focus::MENU_EDITOR_TOOLS,
                        onclick: move |_| {
                            if *open_menu.read() == OpenMenu::EditorTools {
                                close_editor_tools_menu();
                            } else {
                                open_editor_tools_menu();
                            }
                        },
                        onkeydown: move |event: KeyboardEvent| {
                            let Some(target) = shell_focus::trigger_key_intent(&event.key()) else {
                                return;
                            };
                            event.prevent_default();
                            event.stop_propagation();
                            if *open_menu.read() != OpenMenu::EditorTools {
                                open_editor_tools_menu();
                            }
                            shell_focus::focus_menu_item(shell_focus::MENU_EDITOR_TOOLS, target);
                        },
                        "•••"
                    }
                    if adv_open {
                    div { id: shell_focus::MENU_EDITOR_TOOLS, class: "adv-dropdown", role: "menu",
                        // Split
                        button {
                            class: if mode == EditorMode::Split { "dropdown-item active" } else { "dropdown-item" },
                            role: "menuitem",
                            tabindex: "-1",
                            "data-source-focus-launch": "mode-split",
                            onclick: move |_| {
                                let target = if mode == EditorMode::Split {
                                    EditorMode::Text
                                } else {
                                    EditorMode::Split
                                };
                                submit_source_interaction(
                                    source_sync,
                                    state,
                                    mode_sig,
                                    toasts,
                                    SourceCommand::SwitchMode(target),
                                    SourceInteractionOrigin::removable_menu_control("mode-split"),
                                    move || open_menu.set(OpenMenu::None),
                                );
                            },
                            if mode == EditorMode::Split {
                                {tr(ui_lang, "mode.close_split")}
                            } else {
                                {tr(ui_lang, "mode.split")}
                            }
                        }
                        hr { class: "dropdown-sep" }
                        // Outline
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            tabindex: "-1",
                            onclick: move |_| {
                                let o = *outline_open.read();
                                outline_open.set(!o);
                                search_open.set(false);
                                backlinks_open.set(false);
                                history_open.set(false);
                                close_editor_tools_menu();
                            },
                            {tr(ui_lang, "outline.title")}
                        }
                        if backlinks_available {
                            // Backlinks
                            button {
                                class: "dropdown-item",
                                role: "menuitem",
                                tabindex: "-1",
                                onclick: move |_| {
                                    let o = *backlinks_open.read();
                                    backlinks_open.set(!o);
                                    outline_open.set(false);
                                    search_open.set(false);
                                    history_open.set(false);
                                    close_editor_tools_menu();
                                },
                                {tr(ui_lang, "backlinks.title")}
                            }
                        }
                        // History
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            tabindex: "-1",
                            onclick: move |_| {
                                let o = *history_open.read();
                                history_open.set(!o);
                                outline_open.set(false);
                                search_open.set(false);
                                backlinks_open.set(false);
                                close_editor_tools_menu();
                            },
                            {tr(ui_lang, "history.title")}
                        }
                        hr { class: "dropdown-sep" }
                        // Export HTML
                        button {
                            class: "dropdown-item",
                            role: "menuitem",
                            tabindex: "-1",
                            onclick: move |_| {
                                close_editor_tools_menu();
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
}
