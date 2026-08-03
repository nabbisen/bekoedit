//! Workspace explorer sidebar.
//!
//! Uses the `DirectoryTree` state machine from dioxus-swdir-tree for lazy
//! directory loading, but renders each row itself with a plain `onclick`
//! handler.  This avoids the drag-and-drop system whose `is_drag_active`
//! prop is captured at last-render time and creates a race condition in
//! Dioxus Desktop where fast clicks (mousedown → mouseup before the next
//! repaint) never resolve to `DragOutcome::Clicked`.
//!
//! For bekoedit's needs — click to open, click to expand — drag-and-drop
//! is not required.

use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_swdir_tree::{DirectoryTree, ThreadExecutor, use_scan_driver};

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::{search_panel::SearchPanel, toast::Toast};
use crate::i18n::{Lang, tr};
use crate::shell_focus;
use crate::source_sync::{SourceSyncState, cancel_source_focus};
use crate::state::{
    BacklinksOpen, HistoryOpen, NewFileOpen, OpenMenu, OpenMenuState, OutlineOpen, SearchOpen,
};

use dioxus_swdir_tree::TreeNode;

mod tree_nav;
mod tree_row;
use tree_row::TreeRowItem;

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let mut source_sync = use_context::<Signal<SourceSyncState>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let mut search_open = use_context::<SearchOpen>().0;
    let mut outline_open = use_context::<OutlineOpen>().0;
    let mut backlinks_open = use_context::<BacklinksOpen>().0;
    let mut history_open = use_context::<HistoryOpen>().0;
    let mut open_menu = use_context::<OpenMenuState>().0;

    let root = state.read().workspace.as_ref().map(|w| w.root_path.clone());
    let Some(root_path) = root else {
        return rsx! {
            aside { class: "explorer",
                p { class: "muted", {tr(ui_lang, "explorer.no_workspace")} }
            }
        };
    };

    let root_memo = use_memo(move || root_path.clone());
    let mut tree_sig = use_signal(|| DirectoryTree::new(root_memo()));
    let scan_ch = use_scan_driver(tree_sig, Arc::new(ThreadExecutor));

    // Auto-expand root on mount.
    {
        let sc = scan_ch;
        use_effect(move || {
            if let Some(req) = tree_sig.write().on_toggled(&root_memo()) {
                sc.send(req);
            }
        });
    }

    // ── New-file form ──────────────────────────────────────────────────────
    let mut new_name = use_signal(String::new);
    // Shared context, not local state (RFC-042 K1): the app-menu and
    // editor-tools-menu triggers must be able to close this disclosure to
    // keep the four shell-authority-holding surfaces mutually exclusive.
    let mut show_new = use_context::<NewFileOpen>().0;
    let mut form_error = use_signal(String::new);
    let templates = state.read().list_templates();
    let mut tpl_content = use_signal(String::new);

    use_effect(move || {
        if *show_new.read() {
            document::eval(
                r#"requestAnimationFrame(() => document.getElementById('workspace-new-file-name')?.focus())"#,
            );
        }
    });

    let mut do_create = move || {
        let name = new_name.read().clone();
        let content = tpl_content.read().clone();
        let result = if content.is_empty() {
            state
                .write()
                .create_markdown_file(&PathBuf::new(), &name)
                .map(|_| ())
        } else {
            state
                .write()
                .create_from_template(&PathBuf::new(), &name, &content)
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                form_error.set(String::new());
                show_new.set(false);
                new_name.set(String::new());
                source_sync.write().release_shell_focus();
                shell_focus::focus_element(shell_focus::TRIGGER_NEW_FILE);
            }
            Err(e) => form_error.set(e.to_string()),
        }
        // Reload tree after creating a file.
        *tree_sig.write() = DirectoryTree::new(root_memo());
        if let Some(req) = tree_sig.write().on_toggled(&root_memo()) {
            scan_ch.send(req);
        }
    };

    // ── Collect visible rows for rendering ─────────────────────────────────
    let rows: Vec<(TreeNode, u32)> = tree_sig
        .read()
        .visible_rows()
        .into_iter()
        .map(|(n, d)| (n.clone(), d))
        .collect();

    // Roving tabindex target, tracked by path so it survives a rescan,
    // expand, collapse, or refresh that renumbers rows (RFC-042 §7.2).
    let active_path = use_signal(|| None::<PathBuf>);
    let active_index = match active_path.read().as_deref() {
        Some(tracked) => {
            let paths: Vec<PathBuf> = rows.iter().map(|(n, _)| n.path.clone()).collect();
            tree_nav::resolve_active_row(&paths, tracked)
        }
        None => (!rows.is_empty()).then_some(0),
    };
    // The open document's row is "selected" — distinct from "active", the
    // roving focus target (RFC-042 §7.3).
    let selected_path = state.read().session.as_ref().map(|s| s.path.clone());

    rsx! {
        aside { class: "explorer", role: "complementary", aria_label: tr(ui_lang, "explorer.label"),

            // ── Toolbar ──────────────────────────────────────────────────
            div { class: "explorer-toolbar",
                button {
                    id: shell_focus::TRIGGER_WORKSPACE_SEARCH,
                    class: if *search_open.read() { "explorer-tool-btn active" } else { "explorer-tool-btn" },
                    aria_label: tr(ui_lang, "search.label"),
                    aria_controls: "workspace-search-panel",
                    aria_expanded: "{search_open}",
                    title: tr(ui_lang, "search.label"),
                    onclick: move |_| {
                        let next = !*search_open.read();
                        if next {
                            cancel_source_focus(source_sync);
                        } else {
                            source_sync.write().release_shell_focus();
                            shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH);
                        }
                        search_open.set(next);
                        outline_open.set(false);
                        backlinks_open.set(false);
                        history_open.set(false);
                        open_menu.set(OpenMenu::None);
                        // RFC-042 K1: the four shell-authority-holding
                        // surfaces (both menus, search, new-file) stay
                        // mutually exclusive.
                        show_new.set(false);
                    },
                    {tr(ui_lang, "search.label")}
                }
                div { class: "explorer-toolbar-spacer" }
                button {
                    id: shell_focus::TRIGGER_NEW_FILE,
                    class: if *show_new.read() { "icon-btn active" } else { "icon-btn" },
                    title: tr(ui_lang, "explorer.new_file"),
                    aria_label: tr(ui_lang, "explorer.new_file"),
                    aria_controls: "workspace-new-file-form",
                    aria_expanded: "{show_new}",
                    onclick: move |_| {
                        let next = !*show_new.read();
                        if next {
                            cancel_source_focus(source_sync);
                            // RFC-042 K1: the four shell-authority-holding
                            // surfaces (both menus, search, new-file) stay
                            // mutually exclusive.
                            search_open.set(false);
                            open_menu.set(OpenMenu::None);
                        } else {
                            source_sync.write().release_shell_focus();
                            shell_focus::focus_element(shell_focus::TRIGGER_NEW_FILE);
                        }
                        show_new.set(next);
                        if !next {
                            form_error.set(String::new());
                        }
                    },
                    "+"
                }
            }

            // ── New-file form ─────────────────────────────────────────────
            if *search_open.read() {
                SearchPanel {}
            }

            if *show_new.read() {
                div { id: "workspace-new-file-form", class: "new-file-row",
                    input {
                        id: "workspace-new-file-name",
                        r#type: "text",
                        placeholder: "filename.md",
                        aria_label: tr(ui_lang, "explorer.new_file_name"),
                        value: "{new_name}",
                        oninput:   move |e| new_name.set(e.value()),
                        onkeydown: move |e| match e.key() {
                            Key::Enter => do_create(),
                            Key::Escape => {
                                source_sync.write().release_shell_focus();
                                shell_focus::focus_element(shell_focus::TRIGGER_NEW_FILE);
                                show_new.set(false);
                                form_error.set(String::new());
                            }
                            _ => {}
                        },
                    }
                    if !templates.is_empty() {
                        select {
                            class: "template-select",
                            aria_label: tr(ui_lang, "templates.label"),
                            onchange: move |e| {
                                let v = e.value();
                                tpl_content.set(if v == "__blank__" { String::new() } else { v });
                            },
                            option { value: "__blank__", {tr(ui_lang, "templates.blank")} }
                            for t in &templates {
                                option { value: "{t.content}", "{t.name}" }
                            }
                        }
                    }
                    button { class: "btn-primary", onclick: move |_| do_create(), {tr(ui_lang, "explorer.create")} }
                    button {
                        class: "new-file-close",
                        aria_label: tr(ui_lang, "explorer.cancel_new_file"),
                        title: tr(ui_lang, "explorer.cancel_new_file"),
                        onclick: move |_| {
                            source_sync.write().release_shell_focus();
                            shell_focus::focus_element(shell_focus::TRIGGER_NEW_FILE);
                            show_new.set(false);
                            form_error.set(String::new());
                        },
                        "×"
                    }
                }
                if !form_error.read().is_empty() {
                    p { class: "error-inline", "{form_error}" }
                }
            }

            // ── Tree rows (custom renderer, no drag) ───────────────────────
            div { class: "tree-rows", role: "tree",
                for (index , (node , depth)) in rows.into_iter().enumerate() {
                    TreeRowItem {
                        key: "{node.path.display()}",
                        is_active: active_index == Some(index),
                        is_selected: selected_path.as_deref() == Some(node.path.as_path()),
                        node,
                        depth,
                        index,
                        root: root_memo(),
                        tree_sig,
                        scan_ch,
                        state,
                        mode_sig,
                        source_sync,
                        toasts,
                        active_path,
                    }
                }
            }
        }
    }
}
