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

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};

use dioxus_swdir_tree::{ScanRequest, TreeNode};

#[component]
pub fn Explorer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();
    let toasts = use_context::<Signal<Vec<Toast>>>();

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
    let mut show_new = use_signal(|| false);
    let mut form_error = use_signal(String::new);
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
                form_error.set(String::new());
                show_new.set(false);
                new_name.set(String::new());
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

    rsx! {
        aside { class: "explorer", role: "complementary", aria_label: tr(ui_lang, "explorer.label"),

            // ── Toolbar ──────────────────────────────────────────────────
            div { class: "explorer-toolbar",
                button {
                    class: "icon-btn",
                    title: tr(ui_lang, "explorer.new_file"),
                    onclick: move |_| { let v = *show_new.read(); show_new.set(!v); },
                    "+"
                }
            }

            // ── New-file form ─────────────────────────────────────────────
            if *show_new.read() {
                div { class: "new-file-row",
                    input {
                        r#type: "text",
                        placeholder: "filename.md",
                        aria_label: tr(ui_lang, "explorer.new_file_name"),
                        value: "{new_name}",
                        oninput:   move |e| new_name.set(e.value()),
                        onkeydown: move |e| { if e.key() == Key::Enter { do_create(); } },
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
                }
                if !form_error.read().is_empty() {
                    p { class: "error-inline", "{form_error}" }
                }
            }

            // ── Tree rows (custom renderer, no drag) ───────────────────────
            div { class: "tree-rows", role: "tree",
                for (node, depth) in rows {
                    TreeRowItem {
                        key: "{node.path.display()}",
                        node,
                        depth,
                        root: root_memo(),
                        tree_sig,
                        scan_ch,
                        state,
                        toasts,
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TreeRowItemProps {
    node: TreeNode,
    depth: u32,
    root: PathBuf,
    tree_sig: Signal<DirectoryTree>,
    scan_ch: Coroutine<ScanRequest>,
    state: Signal<AppState>,
    toasts: Signal<Vec<Toast>>,
}

#[component]
fn TreeRowItem(props: TreeRowItemProps) -> Element {
    let TreeRowItemProps {
        node,
        depth,
        root,
        mut tree_sig,
        scan_ch,
        mut state,
        mut toasts,
    } = props;

    let indent_px = depth * 16;
    let path = node.path.clone();
    let is_dir = node.is_dir;
    let name = node
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.path.display().to_string());

    let (icon, row_class) = if is_dir {
        let arrow = if node.is_expanded { "▾" } else { "▸" };
        (arrow, "tree-row tree-dir")
    } else {
        ("·", "tree-row tree-file")
    };

    rsx! {
        div {
            class: row_class,
            style: "padding-left: {indent_px}px",
            role: "treeitem",
            aria_expanded: if is_dir { "{node.is_expanded}" } else { "false" },
            onclick: move |_| {
                if is_dir {
                    if let Some(req) = tree_sig.write().on_toggled(&path) {
                        scan_ch.send(req);
                    }
                } else {
                    let rel = path.strip_prefix(&root)
                        .map(|r| r.to_path_buf())
                        .unwrap_or_else(|_| path.clone());
                    match state.write().open_document(&rel) {
                        Ok(()) => {}
                        Err(e) => push_toast(&mut toasts, ToastKind::Error, e.to_string()),
                    }
                }
            },
            span { class: "tree-icon", "{icon} " }
            span { class: "tree-name", "{name}" }
        }
    }
}
