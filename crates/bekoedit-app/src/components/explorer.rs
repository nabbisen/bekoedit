//! Workspace explorer sidebar using dioxus-swdir-tree (RFC-004/005).
//!
//! Replaces the hand-rolled recursive scanner with DirectoryTreeView, which
//! provides lazy loading (one level per expansion), built-in keyboard
//! navigation, and the DEFAULT_PREFETCH_SKIP list (.git, node_modules,
//! target …). File operations (create, rename, delete) are surfaced as a
//! compact toolbar above the tree.

use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use dioxus_swdir_tree::{
    DirectoryTree, DirectoryTreeEvent, DirectoryTreeView, DragOutcome, SelectionMode,
    ThreadExecutor, use_scan_driver,
};

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};

/// Markdown file extensions the explorer opens as documents.
fn is_markdown(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("md") | Some("markdown")
    )
}

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let _toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();

    // ── Tree state ─────────────────────────────────────────────────────────
    let root = state.read().workspace.as_ref().map(|w| w.root_path.clone());
    let Some(root_path) = root else {
        return rsx! { aside { class: "explorer", p { class: "muted", {tr(lang, "explorer.no_workspace")} } } };
    };

    let root_sig = use_memo(move || root_path.clone());
    let mut tree_sig = use_signal(|| DirectoryTree::new(root_sig()));
    let scan_ch = use_scan_driver(tree_sig, Arc::new(ThreadExecutor));

    let on_tree_event = move |ev: DirectoryTreeEvent| {
        match ev {
            DirectoryTreeEvent::Toggled(path) => {
                if let Some(req) = tree_sig.write().on_toggled(&path) {
                    scan_ch.send(req);
                }
            }
            DirectoryTreeEvent::Selected { path, is_dir, mode } => {
                tree_sig.write().on_selected(&path, is_dir, mode);
                if !is_dir && is_markdown(&path) {
                    // Open the document in AppState (workspace-relative path)
                    if let Ok(rel) = path.strip_prefix(root_sig()) {
                        let _ = state.write().open_document(rel);
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
    };

    // ── File operation toolbar state ────────────────────────────────────────
    let mut new_name = use_signal(String::new);
    let mut show_new = use_signal(|| false);
    let mut error = use_signal(String::new);
    let templates = state.read().list_templates();
    let mut tpl_content = use_signal(String::new);

    let mut run = move |result: Result<(), String>| {
        if let Err(e) = result {
            error.set(e);
        } else {
            error.set(String::new());
            show_new.set(false);
            new_name.set(String::new());
        }
    };

    rsx! {
        aside { class: "explorer",
            role: "complementary",
            aria_label: tr(lang, "explorer.label"),

            // ── File operations toolbar ─────────────────────────────────────
            div { class: "explorer-toolbar",
                button {
                    class: "icon-btn",
                    title: tr(lang, "explorer.new_file"),
                    onclick: move |_| show_new.set(!show_new()),
                    "+"
                }
                button {
                    class: "icon-btn",
                    title: tr(lang, "explorer.refresh"),
                    onclick: move |_| {
                        // Re-create the tree to force a full refresh
                        *tree_sig.write() = DirectoryTree::new(root_sig());
                    },
                    "↻"
                }
            }

            // ── New file row ────────────────────────────────────────────────
            if *show_new.read() {
                div { class: "new-file-row",
                    input {
                        r#type: "text",
                        placeholder: "filename.md",
                        aria_label: tr(lang, "explorer.new_file_name"),
                        value: "{new_name}",
                        oninput: move |e| new_name.set(e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                let name = new_name.read().clone();
                                let content = tpl_content.read().clone();
                                let res = if content.is_empty() {
                                    state.write()
                                        .create_markdown_file(&PathBuf::new(), &name)
                                        .map(|_| ())
                                        .map_err(|e| e.to_string())
                                } else {
                                    state.write()
                                        .create_from_template(&PathBuf::new(), &name, &content)
                                        .map(|_| ())
                                        .map_err(|e| e.to_string())
                                };
                                run(res);
                                // Refresh tree
                                *tree_sig.write() = DirectoryTree::new(root_sig());
                            }
                        },
                    }
                    if !templates.is_empty() {
                        select {
                            class: "template-select",
                            aria_label: tr(lang, "templates.label"),
                            onchange: move |evt| {
                                let val = evt.value();
                                tpl_content.set(if val == "__blank__" { String::new() } else { val });
                            },
                            option { value: "__blank__", {tr(lang, "templates.blank")} }
                            for tpl in &templates {
                                option { value: "{tpl.content}", "{tpl.name}" }
                            }
                        }
                    }
                    button {
                        class: "btn-primary",
                        onclick: move |_| {
                            let name = new_name.read().clone();
                            let content = tpl_content.read().clone();
                            let res = if content.is_empty() {
                                state.write()
                                    .create_markdown_file(&PathBuf::new(), &name)
                                    .map(|_| ())
                                    .map_err(|e| e.to_string())
                            } else {
                                state.write()
                                    .create_from_template(&PathBuf::new(), &name, &content)
                                    .map(|_| ())
                                    .map_err(|e| e.to_string())
                            };
                            run(res);
                            *tree_sig.write() = DirectoryTree::new(root_sig());
                        },
                        {tr(lang, "explorer.create")}
                    }
                }
                if !error.read().is_empty() {
                    p { class: "error-inline", "{error}" }
                }
            }

            // ── dioxus-swdir-tree DirectoryTreeView ────────────────────────
            DirectoryTreeView { tree: tree_sig, on_event: on_tree_event }
        }
    }
}
