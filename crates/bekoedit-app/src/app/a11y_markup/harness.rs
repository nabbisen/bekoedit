//! Renders the shell's real components headless, with `dioxus-ssr`, inside the
//! same contexts `App` provides (RFC-050 slice 1, level 2). It renders no
//! WebView: `App` itself needs the launch configuration and the real settings
//! directory, so this provides its own contexts over a throwaway workspace and an
//! isolated profile, and renders the components `App` renders.
//!
//! `dioxus-ssr` renders what a first render produces. It does not run effects or
//! the async tree scan, so the explorer's rows are rendered separately, from
//! constructed nodes (`explorer_rows`); that is said where it matters.

use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::{HistoryStore, RecoveryStore};
use bekoedit_ui_contract::EditorMode;

use super::html::{self, Node};
use crate::components::{
    app_bar::AppBar,
    settings_screen::SettingsScreen,
    start_screen::StartScreen,
    toast::{Toast, ToastKind, ToastLayer},
};
use crate::i18n::Lang;
use crate::persistence::AppPersistence;
use crate::source_sync::SourceSyncState;
use crate::state::{
    BacklinksOpen, ExplorerCollapsed, HistoryOpen, MenuEntryIntent, NewFileOpen, OpenMenu,
    OpenMenuState, OutlineOpen, SearchOpen, SettingsOpen,
};

/// `note.md` is the open document: the very text level 3 seeds and opens
/// (`webview_smoke::A11Y_NOTE`), one block of every kind Form Mode shows as a
/// field. `other.md` is a second file in the tree.
pub(super) const NOTE: &str = crate::webview_smoke::A11Y_NOTE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Screen {
    /// The Start Screen: no workspace.
    Start,
    /// The shell with a workspace and no document.
    NoDocument,
    /// The shell with `note.md` open.
    Document,
    /// Settings, over the shell.
    Settings,
}

/// What else to put on the page.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Opts {
    /// `note.md` edited, so the save state is not clean.
    pub(super) dirty: bool,
    /// One toast showing.
    pub(super) toast: bool,
    /// Which overflow menu is open.
    pub(super) menu: OpenMenu,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            dirty: false,
            toast: false,
            menu: OpenMenu::None,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Scene {
    lang: Lang,
    mode: EditorMode,
    screen: Screen,
    opts: Opts,
    dir: PathBuf,
}

/// A throwaway workspace and profile directory with the two files in it.
pub(super) fn scratch() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch directory");
    std::fs::create_dir(dir.path().join("workspace")).unwrap();
    std::fs::write(dir.path().join("workspace/note.md"), NOTE).unwrap();
    std::fs::write(dir.path().join("workspace/other.md"), "# Other\n").unwrap();
    dir
}

#[allow(non_snake_case)] // a Dioxus root component, named like one
fn Root(scene: Scene) -> Element {
    let profile = scene.dir.join("profile");
    use_context_provider(|| AppPersistence::isolated(profile.clone()));
    let workspace = scene.dir.join("workspace");
    let state = use_hook(|| {
        let mut app = AppState::new_with_history(
            RecoveryStore::at(scene.dir.join("recovery")),
            scene.dir.join("recent.json"),
            HistoryStore::at(scene.dir.join("history")),
            1_500,
        );
        if scene.screen != Screen::Start {
            app.open_workspace(&workspace, 0)
                .expect("the scratch workspace opens");
        }
        if matches!(scene.screen, Screen::Document | Screen::Settings) {
            app.open_document(Path::new("note.md"))
                .expect("note.md opens");
            if scene.opts.dirty {
                let revision = app.session.as_ref().unwrap().revision;
                app.edit_text(revision, format!("{NOTE}more\n"), 0)
                    .expect("an edit");
            }
        }
        Signal::new(app)
    });
    use_context_provider(|| state);
    use_context_provider(|| Signal::new(scene.lang));
    use_context_provider(|| Signal::new(scene.mode));
    use_context_provider(|| ExplorerCollapsed(Signal::new(false)));
    use_context_provider(|| SettingsOpen(Signal::new(scene.screen == Screen::Settings)));
    use_context_provider(|| OutlineOpen(Signal::new(false)));
    use_context_provider(|| SearchOpen(Signal::new(false)));
    use_context_provider(|| BacklinksOpen(Signal::new(false)));
    use_context_provider(|| HistoryOpen(Signal::new(false)));
    use_context_provider(|| NewFileOpen(Signal::new(false)));
    use_context_provider(|| OpenMenuState(Signal::new(scene.opts.menu)));
    use_context_provider(|| MenuEntryIntent(Signal::new(None)));
    use_context_provider(|| {
        let mut toasts = Vec::new();
        if scene.opts.toast {
            toasts.push(Toast {
                id: 1,
                kind: ToastKind::Info,
                message: "A notice".into(),
            });
        }
        Signal::new(toasts)
    });
    use_context_provider(|| Signal::new(SourceSyncState::default()));
    rsx! {
        ToastLayer {}
        div { class: "app-frame",
            AppBar {}
            match scene.screen {
                Screen::Start => rsx! { StartScreen {} },
                Screen::Settings => rsx! { SettingsScreen {} },
                Screen::NoDocument | Screen::Document => rsx! { super::super::MainShell {} },
            }
        }
    }
}

/// One rendered page.
pub(super) struct Page {
    pub(super) nodes: Vec<Node>,
}

impl Page {
    pub(super) fn all(&self) -> Vec<&html::Element> {
        html::elements(&self.nodes)
    }

    /// Elements with this ARIA role.
    pub(super) fn with_role(&self, role: &str) -> Vec<&html::Element> {
        self.all()
            .into_iter()
            .filter(|e| e.role().as_deref() == Some(role))
            .collect()
    }

    pub(super) fn name_of(&self, element: &html::Element) -> Option<String> {
        html::accessible_name(&self.nodes, element)
    }

    /// The elements with this role, as `(role, name)`-style names, for messages.
    pub(super) fn names_with_role(&self, role: &str) -> Vec<String> {
        self.with_role(role)
            .into_iter()
            .map(|e| self.name_of(e).unwrap_or_else(|| "<no name>".into()))
            .collect()
    }

    /// The one element with this role and exactly this accessible name.
    pub(super) fn find(&self, role: &str, name: &str) -> Option<&html::Element> {
        self.with_role(role)
            .into_iter()
            .find(|e| self.name_of(e).as_deref() == Some(name))
    }

    pub(super) fn describe(&self) -> String {
        let mut roles: Vec<String> = self
            .all()
            .iter()
            .filter_map(|e| {
                e.role().map(|r| {
                    format!(
                        "{r}:{}",
                        self.name_of(e).unwrap_or_else(|| "<no name>".into())
                    )
                })
            })
            .collect();
        roles.dedup();
        roles.join(" | ")
    }
}

pub(super) fn render(lang: Lang, mode: EditorMode, screen: Screen, opts: Opts) -> Page {
    let dir = scratch();
    let scene = Scene {
        lang,
        mode,
        screen,
        opts,
        dir: dir.path().to_path_buf(),
    };
    let mut dom = VirtualDom::new_with_props(Root, scene);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    let nodes = html::parse(&html);
    Page { nodes }
}
