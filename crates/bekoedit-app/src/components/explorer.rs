//! Workspace explorer sidebar (RFC-004 tree projection, RFC-005 file ops).
//!
//! Rows are projections (`FileTreeNode`); clicking a Markdown file opens it
//! through the store, which performs path scoping. Rename and delete act on
//! the selected row; deletion goes to the OS trash.

use std::path::PathBuf;

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::{DeleteStrategy, FileNodeKind};

use crate::i18n::{Lang, tr};

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut new_name = use_signal(String::new);
    let mut rename_to = use_signal(String::new);
    let mut selected = use_signal(|| Option::<PathBuf>::None);
    let mut error = use_signal(String::new);

    let nodes = state.read().tree.nodes.clone();
    let workspace_name = state
        .read()
        .workspace
        .as_ref()
        .map(|w| w.display_name.clone())
        .unwrap_or_default();

    let mut run = move |result: Result<(), String>| match result {
        Ok(()) => error.set(String::new()),
        Err(e) => error.set(e),
    };

    rsx! {
        aside { class: "explorer",
            h2 { class: "workspace-name", "{workspace_name}" }
            div { class: "new-file-row",
                input {
                    r#type: "text",
                    placeholder: tr(lang, "explorer.name_placeholder"),
                    value: "{new_name}",
                    oninput: move |evt| new_name.set(evt.value()),
                }
                button {
                    onclick: move |_| {
                        let name = new_name.read().clone();
                        let result = state
                            .write()
                            .create_markdown_file(&PathBuf::new(), &name)
                            .map(|_| new_name.set(String::new()))
                            .map_err(|e| e.to_string());
                        run(result);
                    },
                    {tr(lang, "explorer.new_file")}
                }
            }
            if !error.read().is_empty() {
                p { class: "error", "{error}" }
            }
            if nodes.is_empty() {
                p { class: "muted", {tr(lang, "explorer.empty")} }
            }
            ul { class: "tree",
                for node in nodes {
                    li {
                        class: if node.kind == FileNodeKind::Directory { "dir" } else { "file" },
                        class: if *selected.read() == Some(node.relative_path.clone()) { "selected" },
                        style: "padding-left: {node.depth as u32 * 14 + 8}px",
                        onclick: {
                            let path = node.relative_path.clone();
                            let kind = node.kind;
                            move |_| {
                                selected.set(Some(path.clone()));
                                rename_to.set(String::new());
                                if kind == FileNodeKind::MarkdownFile {
                                    let result = state
                                        .write()
                                        .open_document(&path)
                                        .map_err(|e| e.to_string());
                                    run(result);
                                }
                            }
                        },
                        span { "{node.display_name}" }
                    }
                }
            }
            if let Some(path) = selected.read().clone() {
                div { class: "node-actions",
                    input {
                        r#type: "text",
                        placeholder: tr(lang, "explorer.name_placeholder"),
                        value: "{rename_to}",
                        oninput: move |evt| rename_to.set(evt.value()),
                    }
                    button {
                        onclick: {
                            let path = path.clone();
                            move |_| {
                                let name = rename_to.read().clone();
                                let result = state
                                    .write()
                                    .rename_path(&path, &name)
                                    .map(|renamed| selected.set(Some(renamed)))
                                    .map_err(|e| e.to_string());
                                run(result);
                            }
                        },
                        {tr(lang, "explorer.rename")}
                    }
                    button {
                        class: "danger",
                        onclick: {
                            let path = path.clone();
                            move |_| {
                                let result = state
                                    .write()
                                    .delete_path(&path, DeleteStrategy::MoveToTrash)
                                    .map(|()| selected.set(None))
                                    .map_err(|e| e.to_string());
                                run(result);
                            }
                        },
                        {tr(lang, "explorer.delete")}
                    }
                }
            }
        }
    }
}
