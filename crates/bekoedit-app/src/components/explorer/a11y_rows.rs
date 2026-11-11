//! RFC-050 slice 1, level 2: the explorer's rows. `dioxus-ssr` renders a first
//! pass only, so the explorer's own rows (which arrive from an asynchronous scan)
//! are not on a headless page; each `TreeRowItem` is rendered here directly, from
//! a constructed node, which is all a row's accessible name and state depend on.

use std::path::PathBuf;

use dioxus::prelude::*;
use dioxus_swdir_tree::{DirectoryTree, ScanRequest, TreeNode};

use bekoedit_core::AppState;
use bekoedit_fs::{HistoryStore, RecoveryStore};
use bekoedit_ui_contract::EditorMode;

use super::tree_row::TreeRowItem;
use crate::app::a11y_markup::html::{self, parse};
use crate::components::toast::Toast;
use crate::source_sync::SourceSyncState;

fn node(path: &str, is_dir: bool, expanded: bool) -> TreeNode {
    TreeNode {
        path: PathBuf::from(path),
        is_dir,
        is_expanded: expanded,
        is_loaded: true,
        is_selected: false,
        children: vec![],
        error: None,
    }
}

#[derive(Clone, PartialEq)]
struct Rows {
    dir: PathBuf,
    selected: usize,
}

fn rows_root(rows: Rows) -> Element {
    let state = use_hook(|| {
        Signal::new(AppState::new_with_history(
            RecoveryStore::at(rows.dir.join("recovery")),
            rows.dir.join("recent.json"),
            HistoryStore::at(rows.dir.join("history")),
            1_500,
        ))
    });
    let tree_sig = use_signal(|| DirectoryTree::new("/ws"));
    let scan_ch = use_coroutine(|_rx: UnboundedReceiver<ScanRequest>| async move {});
    let mode_sig = use_signal(|| EditorMode::Text);
    let source_sync = use_signal(SourceSyncState::default);
    let toasts = use_signal(Vec::<Toast>::new);
    let active_path = use_signal(|| None::<PathBuf>);
    let nodes = [
        node("/ws", true, true),
        node("/ws/sub", true, false),
        node("/ws/note.md", false, false),
        node("/ws/other.md", false, false),
        node("/ws/pic.png", false, false),
    ];
    rsx! {
        for (index , n) in nodes.into_iter().enumerate() {
            TreeRowItem {
                depth: if index == 0 { 0 } else { 1 },
                index,
                is_active: index == 0,
                is_selected: index == rows.selected,
                root: PathBuf::from("/ws"),
                tree_sig,
                scan_ch,
                state,
                mode_sig,
                source_sync,
                toasts,
                active_path,
                node: n,
            }
        }
    }
}

fn render_rows(selected: usize) -> (Vec<html::Node>, String) {
    let scratch = tempfile::tempdir().unwrap();
    let mut dom = VirtualDom::new_with_props(
        rows_root,
        Rows {
            dir: scratch.path().to_path_buf(),
            selected,
        },
    );
    dom.rebuild_in_place();
    let markup = dioxus_ssr::render(&dom);
    (parse(&markup), markup)
}

#[test]
fn a_row_is_a_treeitem_named_by_its_file_name_alone() {
    let (page, markup) = render_rows(2);
    let rows: Vec<_> = html::elements(&page)
        .into_iter()
        .filter(|e| e.role().as_deref() == Some("treeitem"))
        .collect();
    assert_eq!(rows.len(), 5, "{markup}");
    let names: Vec<String> = rows
        .iter()
        .map(|r| html::accessible_name(&page, r).unwrap_or_default())
        .collect();
    assert_eq!(
        names,
        ["ws", "sub", "note.md", "other.md", "pic.png"],
        "an icon glyph must not be part of a row's name"
    );
}

#[test]
fn exactly_the_open_documents_row_is_selected_and_each_row_says_whether_it_can_be_opened() {
    let (page, markup) = render_rows(2);
    let rows: Vec<_> = html::elements(&page)
        .into_iter()
        .filter(|e| e.role().as_deref() == Some("treeitem"))
        .collect();
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| r.attr("aria-selected") == Some("true"))
        .map(|r| r.attr("title"))
        .collect();
    assert_eq!(selected, [Some("note.md")], "{markup}");
    let disabled: Vec<_> = rows
        .iter()
        .filter(|r| r.attr("aria-disabled") == Some("true"))
        .map(|r| r.attr("title"))
        .collect();
    assert_eq!(
        disabled,
        [Some("pic.png")],
        "only the file the editor cannot open is aria-disabled"
    );
    assert_eq!(rows[0].attr("aria-expanded"), Some("true"));
    assert_eq!(rows[1].attr("aria-expanded"), Some("false"));
}
