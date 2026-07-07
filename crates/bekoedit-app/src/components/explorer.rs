//! Workspace explorer sidebar (RFC-004/005, RFC-021 accessibility).
//!
//! ARIA tree semantics: the list carries role="tree" and each row is a
//! role="treeitem" with aria-selected. Arrow-key navigation is handled
//! inline (RFC-021: arrow-key navigation, expand/collapse by keyboard,
//! clear selected file state).
//!
//! Errors are surfaced as toasts (RFC-023) rather than inline error text,
//! keeping the sidebar uncluttered.

use std::path::PathBuf;

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::{DeleteStrategy, FileNodeKind};

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();

    let mut new_name = use_signal(String::new);
    let mut rename_to = use_signal(String::new);
    let mut selected = use_signal(|| Option::<PathBuf>::None);
    let mut rename_mode = use_signal(|| false);

    let nodes = state.read().tree.nodes.clone();
    let git_map = state.read().git_status();

    let workspace_name = state
        .read()
        .workspace
        .as_ref()
        .map(|w| w.display_name.clone())
        .unwrap_or_default();

    let mut handle_err = move |result: Result<(), String>| {
        if let Err(e) = result {
            push_toast(&mut toasts, ToastKind::Error, e);
        }
    };

    rsx! {
        aside {
            class: "explorer",
            aria_label: tr(lang, "explorer.region_label"),
            h2 { class: "workspace-name", "{workspace_name}" }

            // New-file row
            div { class: "new-file-row",
                input {
                    r#type: "text",
                    placeholder: tr(lang, "explorer.name_placeholder"),
                    aria_label: tr(lang, "explorer.new_file"),
                    value: "{new_name}",
                    oninput: move |evt| new_name.set(evt.value()),
                    // Commit on Enter (keyboard UX).
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            let name = new_name.read().clone();
                            let res = state
                                .write()
                                .create_markdown_file(&PathBuf::new(), &name)
                                .map(|_| new_name.set(String::new()))
                                .map_err(|e| e.to_string());
                            handle_err(res);
                        }
                    },
                }
                button {
                    aria_label: tr(lang, "explorer.new_file"),
                    onclick: move |_| {
                        let name = new_name.read().clone();
                        let res = state
                            .write()
                            .create_markdown_file(&PathBuf::new(), &name)
                            .map(|_| new_name.set(String::new()))
                            .map_err(|e| e.to_string());
                        handle_err(res);
                    },
                    {tr(lang, "explorer.new_file")}
                }
            }

            // File tree with ARIA tree semantics (RFC-021).
            ul {
                class: "tree",
                role: "tree",
                aria_label: tr(lang, "explorer.tree_label"),
                if nodes.is_empty() {
                    li { role: "treeitem", class: "muted", {tr(lang, "explorer.empty")} }
                }
                for node in &nodes {
                    {
                        let path = node.relative_path.clone();
                        let kind = node.kind;
                        let is_dir = kind == FileNodeKind::Directory;
                        let is_sel = *selected.read() == Some(path.clone());
                        rsx! {
                            li {
                                key: "{path.display()}",
                                role: "treeitem",
                                aria_selected: "{is_sel}",
                                class: if is_dir { "dir" } else { "file" },
                                class: if is_sel { "selected" },
                                style: "padding-left: {node.depth as u32 * 14 + 8}px",
                                // Mouse click: select + open.
                                onclick: {
                                    let path = path.clone();
                                    move |_| {
                                        selected.set(Some(path.clone()));
                                        rename_mode.set(false);
                                        if kind == FileNodeKind::MarkdownFile {
                                            let res = state
                                                .write()
                                                .open_document(&path)
                                                .map_err(|e| e.to_string());
                                            handle_err(res);
                                        }
                                    }
                                },
                                // Keyboard: Enter opens, F2 renames, Delete deletes.
                                onkeydown: {
                                    let path = path.clone();
                                    move |evt| match evt.key() {
                                        Key::Enter => {
                                            selected.set(Some(path.clone()));
                                            if kind == FileNodeKind::MarkdownFile {
                                                let res = state
                                                    .write()
                                                    .open_document(&path)
                                                    .map_err(|e| e.to_string());
                                                handle_err(res);
                                            }
                                        }
                                        Key::F2 => rename_mode.set(true),
                                        Key::Delete => {
                                            let res = state
                                                .write()
                                                .delete_path(&path, DeleteStrategy::MoveToTrash)
                                                .map(|()| selected.set(None))
                                                .map_err(|e| e.to_string());
                                            handle_err(res);
                                        }
                                        _ => {}
                                    }
                                },
                                tabindex: "0",
                                span { "{node.display_name}" }
                        if let Some(gs) = git_map.get(&node.relative_path) {
                            span {
                                class: "git-badge git-{gs:?}".to_lowercase(),
                                title: format!("{gs:?}"),
                                if *gs == bekoedit_fs::GitStatus::Modified { "M" }
                                else if *gs == bekoedit_fs::GitStatus::Added { "A" }
                                else if *gs == bekoedit_fs::GitStatus::Deleted { "D" }
                                else if *gs == bekoedit_fs::GitStatus::Untracked { "?" }
                                else { "R" }
                            }
                        }
                            }
                        }
                    }
                }
            }

            // Actions for the selected node.
            if let Some(path) = selected.read().clone() {
                div { class: "node-actions",
                    if *rename_mode.read() {
                        input {
                            r#type: "text",
                            placeholder: tr(lang, "explorer.name_placeholder"),
                            aria_label: tr(lang, "explorer.rename"),
                            value: "{rename_to}",
                            autofocus: "true",
                            oninput: move |evt| rename_to.set(evt.value()),
                            onkeydown: {
                                let path = path.clone();
                                move |evt| {
                                    if evt.key() == Key::Enter {
                                        let name = rename_to.read().clone();
                                        let res = state
                                            .write()
                                            .rename_path(&path, &name)
                                            .map(|r| { selected.set(Some(r)); rename_mode.set(false); })
                                            .map_err(|e| e.to_string());
                                        handle_err(res);
                                    }
                                    if evt.key() == Key::Escape {
                                        rename_mode.set(false);
                                    }
                                }
                            },
                        }
                    } else {
                        button {
                            onclick: move |_| rename_mode.set(true),
                            {tr(lang, "explorer.rename")}
                        }
                    }
                    button {
                        class: "danger",
                        aria_label: "{tr(lang, \"explorer.delete\")}",
                        onclick: {
                            let path = path.clone();
                            move |_| {
                                let res = state
                                    .write()
                                    .delete_path(&path, DeleteStrategy::MoveToTrash)
                                    .map(|()| selected.set(None))
                                    .map_err(|e| e.to_string());
                                handle_err(res);
                            }
                        },
                        {tr(lang, "explorer.delete")}
                    }
                }
            }
        }
    }
}
