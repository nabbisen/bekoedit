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
use crate::source_sync::{
    SourceCommand, SourceSyncState, cancel_source_focus, submit_source_command,
};
use crate::state::{
    BacklinksOpen, HistoryOpen, NewFileOpen, OpenMenu, OpenMenuState, OutlineOpen, SearchOpen,
};

use dioxus_swdir_tree::{ScanRequest, TreeNode};

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
                for (node, depth) in rows {
                    TreeRowItem {
                        key: "{node.path.display()}",
                        node,
                        depth,
                        root: root_memo(),
                        tree_sig,
                        scan_ch,
                        state,
                        mode_sig,
                        source_sync,
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
    mode_sig: Signal<EditorMode>,
    source_sync: Signal<SourceSyncState>,
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
        state,
        mode_sig,
        source_sync,
        toasts,
    } = props;

    let indent_px = depth * 16;
    let path = node.path.clone();
    let is_dir = node.is_dir;
    let name = node
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.path.display().to_string());

    let is_openable = is_dir || bekoedit_fs::paths::is_markdown_path(&path);
    let (icon, row_class) = if is_dir {
        let arrow = if node.is_expanded { "▾" } else { "▸" };
        (arrow, "tree-row tree-dir")
    } else if is_openable {
        ("·", "tree-row tree-file")
    } else {
        ("·", "tree-row tree-file disabled")
    };

    rsx! {
        button {
            class: row_class,
            style: "padding-left: {indent_px}px",
            role: "treeitem",
            aria_expanded: if is_dir { "{node.is_expanded}" } else { "false" },
            aria_disabled: "{!is_openable}",
            disabled: !is_openable,
            title: "{name}",
            onclick: move |_| {
                if is_dir {
                    if let Some(req) = tree_sig.write().on_toggled(&path) {
                        scan_ch.send(req);
                    }
                } else if is_openable {
                    let rel = path.strip_prefix(&root)
                        .map(|r| r.to_path_buf())
                        .unwrap_or_else(|_| path.clone());
                    submit_source_command(
                        source_sync,
                        state,
                        mode_sig,
                        toasts,
                        SourceCommand::OpenDocument(rel),
                    );
                }
            },
            span { class: "tree-icon", "{icon} " }
            span { class: "tree-name", "{name}" }
        }
    }
}
