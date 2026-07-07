//! Workspace explorer sidebar (RFC-004/005) using dioxus-swdir-tree.
//!
//! Fixes over v0.10.0:
//! - Auto-expands the workspace root on mount so files are immediately visible.
//! - Opens ALL files (not just .md); non-Markdown files open in Text Mode.
//! - Propagates open_document errors as toasts instead of silently ignoring them.

use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_swdir_tree::{
    DirectoryTree, DirectoryTreeEvent, DirectoryTreeView, DragOutcome, SelectionMode,
    ThreadExecutor, use_scan_driver,
};

use bekoedit_core::AppState;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut toasts = use_context::<Signal<Vec<Toast>>>();

    // ── Workspace root ──────────────────────────────────────────────────────
    let root = state.read().workspace.as_ref().map(|w| w.root_path.clone());
    let Some(root_path) = root else {
        return rsx! { aside { class: "explorer", p { class: "muted", {tr(lang, "explorer.no_workspace")} } } };
    };

    // ── Tree state ──────────────────────────────────────────────────────────
    let root_memo = use_memo(move || root_path.clone());
    let mut tree_sig = use_signal(|| DirectoryTree::new(root_memo()));
    let scan_ch = use_scan_driver(tree_sig, Arc::new(ThreadExecutor));

    // Auto-expand the root directory on first mount so files appear immediately.
    {
        let sc = scan_ch;
        use_effect(move || {
            if let Some(req) = tree_sig.write().on_toggled(&root_memo()) {
                sc.send(req);
            }
        });
    }

    // ── Tree event handler ──────────────────────────────────────────────────
    let on_tree_event = {
        let sc = scan_ch;
        move |ev: DirectoryTreeEvent| {
            match ev {
                DirectoryTreeEvent::Toggled(path) => {
                    if let Some(req) = tree_sig.write().on_toggled(&path) {
                        sc.send(req);
                    }
                }
                DirectoryTreeEvent::Selected { path, is_dir, mode } => {
                    tree_sig.write().on_selected(&path, is_dir, mode);
                    if !is_dir {
                        // Strip workspace root to get a relative path, falling back to
                        // the absolute path if strip_prefix fails (e.g. symlink diffs).
                        let rel = path
                            .strip_prefix(root_memo())
                            .map(|r| r.to_path_buf())
                            .unwrap_or_else(|_| path.clone());
                        match state.write().open_document(&rel) {
                            Ok(()) => {}
                            Err(e) => push_toast(&mut toasts, ToastKind::Error, e.to_string()),
                        }
                    }
                }
                DirectoryTreeEvent::Drag(msg) => {
                    let outcome = tree_sig.write().on_drag_msg(msg);
                    if let DragOutcome::Clicked { path, is_dir } = outcome {
                        tree_sig
                            .write()
                            .on_selected(&path, is_dir, SelectionMode::Replace);
                    }
                }
            }
        }
    };

    // ── New-file toolbar ─────────────────────────────────────────────────────
    let mut new_name = use_signal(String::new);
    let mut show_new = use_signal(|| false);
    let mut error = use_signal(String::new);
    let templates = state.read().list_templates();
    let mut tpl_content = use_signal(String::new);

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
                error.set(String::new());
                show_new.set(false);
                new_name.set(String::new());
            }
            Err(e) => error.set(e.to_string()),
        }
        *tree_sig.write() = DirectoryTree::new(root_memo());
        // Re-expand root after refresh
        if let Some(req) = tree_sig.write().on_toggled(&root_memo()) {
            scan_ch.send(req);
        }
    };

    rsx! {
        aside { class: "explorer",
            role: "complementary",
            aria_label: tr(lang, "explorer.label"),

            // ── Toolbar ────────────────────────────────────────────────────
            div { class: "explorer-toolbar",
                button {
                    class: "icon-btn",
                    title: tr(lang, "explorer.new_file"),
                    onclick: move |_| { let v = *show_new.read(); show_new.set(!v); },
                    "+"
                }
                button {
                    class: "icon-btn",
                    title: tr(lang, "explorer.refresh"),
                    onclick: move |_| {
                        *tree_sig.write() = DirectoryTree::new(root_memo());
                        if let Some(req) = tree_sig.write().on_toggled(&root_memo()) { scan_ch.send(req); }
                    },
                    "↻"
                }
            }

            // ── New-file form ────────────────────────────────────────────
            if *show_new.read() {
                div { class: "new-file-row",
                    input {
                        r#type: "text",
                        placeholder: "filename.md",
                        aria_label: tr(lang, "explorer.new_file_name"),
                        value: "{new_name}",
                        oninput:   move |e| new_name.set(e.value()),
                        onkeydown: move |e| { if e.key() == Key::Enter { do_create(); } },
                    }
                    if !templates.is_empty() {
                        select {
                            class: "template-select",
                            aria_label: tr(lang, "templates.label"),
                            onchange: move |e| {
                                let v = e.value();
                                tpl_content.set(if v == "__blank__" { String::new() } else { v });
                            },
                            option { value: "__blank__", {tr(lang, "templates.blank")} }
                            for t in &templates {
                                option { value: "{t.content}", "{t.name}" }
                            }
                        }
                    }
                    button { class: "btn-primary", onclick: move |_| do_create(), {tr(lang, "explorer.create")} }
                }
                if !error.read().is_empty() {
                    p { class: "error-inline", "{error}" }
                }
            }

            // ── dioxus-swdir-tree ─────────────────────────────────────────
            DirectoryTreeView { tree: tree_sig, on_event: on_tree_event }
        }
    }
}
