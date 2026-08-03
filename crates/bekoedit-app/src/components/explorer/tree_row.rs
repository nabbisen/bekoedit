//! One workspace-tree row: WAI-ARIA `treeitem` with roving tabindex
//! (RFC-042 §7.1, handoff slice 2).

use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use dioxus_swdir_tree::{DirectoryTree, ScanRequest, TreeNode};

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::Toast;
use crate::shell_focus;
use crate::source_sync::{SourceCommand, SourceSyncState, submit_source_command};

use super::tree_nav::{self, NavKey, NavOutcome, NavRow};

#[derive(Props, Clone, PartialEq)]
pub(super) struct TreeRowItemProps {
    pub(super) node: TreeNode,
    pub(super) depth: u32,
    /// This row's position in the current `visible_rows()` render — the
    /// `active` argument `tree_nav::navigate` needs.
    pub(super) index: usize,
    /// True for exactly one row at a time: the roving-tabindex target.
    pub(super) is_active: bool,
    /// True for the currently open document's row, when visible. Distinct
    /// from `is_active` — RFC-042 §7.3.
    pub(super) is_selected: bool,
    pub(super) root: PathBuf,
    pub(super) tree_sig: Signal<DirectoryTree>,
    pub(super) scan_ch: Coroutine<ScanRequest>,
    pub(super) state: Signal<AppState>,
    pub(super) mode_sig: Signal<EditorMode>,
    pub(super) source_sync: Signal<SourceSyncState>,
    pub(super) toasts: Signal<Vec<Toast>>,
    /// The tracked active row, by path (RFC-042 §7.2) — this row writes to
    /// it when navigation or a click moves the active target here.
    pub(super) active_path: Signal<Option<PathBuf>>,
}

/// Toggle a directory, open a file, no-op on a non-openable row — the one
/// activation path shared by mouse click and Enter/Space (RFC-042 §7.1). A
/// plain function, not a shared closure, so each call site can capture its
/// own copies of the `Copy` signal/coroutine handles independently.
#[allow(clippy::too_many_arguments)]
fn activate_row(
    is_dir: bool,
    is_openable: bool,
    path: &Path,
    root: &Path,
    mut tree_sig: Signal<DirectoryTree>,
    scan_ch: Coroutine<ScanRequest>,
    state: Signal<AppState>,
    mode_sig: Signal<EditorMode>,
    source_sync: Signal<SourceSyncState>,
    toasts: Signal<Vec<Toast>>,
) {
    if is_dir {
        if let Some(req) = tree_sig.write().on_toggled(path) {
            scan_ch.send(req);
        }
    } else if is_openable {
        let rel = path
            .strip_prefix(root)
            .map(|r| r.to_path_buf())
            .unwrap_or_else(|_| path.to_path_buf());
        submit_source_command(
            source_sync,
            state,
            mode_sig,
            toasts,
            SourceCommand::OpenDocument(rel),
        );
    }
}

#[component]
pub(super) fn TreeRowItem(props: TreeRowItemProps) -> Element {
    let TreeRowItemProps {
        node,
        depth,
        index,
        is_active,
        is_selected,
        root,
        mut tree_sig,
        scan_ch,
        state,
        mode_sig,
        source_sync,
        toasts,
        mut active_path,
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
            "data-tree-row": "",
            tabindex: if is_active { "0" } else { "-1" },
            aria_expanded: if is_dir { "{node.is_expanded}" } else { "false" },
            aria_disabled: "{!is_openable}",
            aria_selected: "{is_selected}",
            title: "{name}",
            onclick: {
                let path = path.clone();
                let root = root.clone();
                move |_| {
                    active_path.set(Some(path.clone()));
                    activate_row(
                        is_dir, is_openable, &path, &root, tree_sig, scan_ch, state, mode_sig,
                        source_sync, toasts,
                    );
                }
            },
            onkeydown: {
                let root = root.clone();
                move |event: KeyboardEvent| {
                    let nav_key = match event.key() {
                        Key::Enter => {
                            event.prevent_default();
                            active_path.set(Some(path.clone()));
                            activate_row(
                                is_dir, is_openable, &path, &root, tree_sig, scan_ch, state,
                                mode_sig, source_sync, toasts,
                            );
                            return;
                        }
                        Key::Character(s) if s == " " => {
                            event.prevent_default();
                            active_path.set(Some(path.clone()));
                            activate_row(
                                is_dir, is_openable, &path, &root, tree_sig, scan_ch, state,
                                mode_sig, source_sync, toasts,
                            );
                            return;
                        }
                        Key::ArrowUp => NavKey::Up,
                        Key::ArrowDown => NavKey::Down,
                        Key::ArrowLeft => NavKey::Left,
                        Key::ArrowRight => NavKey::Right,
                        Key::Home => NavKey::Home,
                        Key::End => NavKey::End,
                        _ => return,
                    };
                    event.prevent_default();

                    // Re-derived fresh from the one canonical source rather
                    // than threading a second, parallel row list through
                    // props — nothing to drift out of sync (handoff §13).
                    let current: Vec<(TreeNode, u32)> = tree_sig
                        .read()
                        .visible_rows()
                        .into_iter()
                        .map(|(n, d)| (n.clone(), d))
                        .collect();
                    let nav_rows: Vec<NavRow> = current
                        .iter()
                        .map(|(n, d)| NavRow {
                            is_dir: n.is_dir,
                            is_expanded: n.is_expanded,
                            depth: *d,
                        })
                        .collect();

                    match tree_nav::navigate(&nav_rows, index, nav_key) {
                        NavOutcome::Move(target) => {
                            if let Some((moved, _)) = current.get(target) {
                                active_path.set(Some(moved.path.clone()));
                                shell_focus::focus_tree_row(target);
                            }
                        }
                        NavOutcome::Expand(target) | NavOutcome::Collapse(target) => {
                            if let Some((toggled, _)) = current.get(target)
                                && let Some(req) = tree_sig.write().on_toggled(&toggled.path)
                            {
                                scan_ch.send(req);
                            }
                        }
                        NavOutcome::None => {}
                    }
                }
            },
            span { class: "tree-icon", "{icon} " }
            span { class: "tree-name", "{name}" }
        }
    }
}
