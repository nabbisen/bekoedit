//! Root application component (RFC-010/019/020/022).

use std::path::PathBuf;

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_fs::{FsWatcher, WatchEvent};
use bekoedit_ui_contract::EditorMode;

use crate::components::{
    conflict_banner::ConflictBanner,
    editor_header::EditorHeader,
    explorer::Explorer,
    form_mode::FormMode,
    outline_panel::OutlinePanel,
    preview_mode::PreviewMode,
    settings_screen::SettingsScreen,
    split_mode::SplitMode,
    start_screen::StartScreen,
    status_bar::StatusBar,
    text_mode::TextMode,
    toast::{Toast, ToastLayer},
};
use crate::i18n::{Lang, tr};
use crate::settings::AppSettings;
use crate::state::{create_app_state, now_ms};

const STYLE: Asset = asset!("/assets/style.css");
const SHORTCUTS_JS: Asset = asset!("/assets/shortcuts.js");
const TICK_MS: u64 = 500;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AppMsg {
    Shortcut { key: String },
}

#[component]
pub fn App() -> Element {
    let settings = AppSettings::load();

    let state = use_context_provider(|| Signal::new(create_app_state()));
    use_context_provider(|| Signal::new(settings.lang));
    use_context_provider(|| Signal::new(settings.default_mode));
    use_context_provider(|| Signal::new(false_val())); // explorer collapsed
    use_context_provider(|| Signal::new(false_val())); // settings screen open
    use_context_provider(|| Signal::new(false_val())); // outline panel open
    use_context_provider(|| Signal::new(Vec::<Toast>::new()));

    // Background task: native fs watcher + autosave + external-change poll.
    // Restarts the watcher whenever the active workspace root changes.
    use_future(move || {
        let mut app: Signal<AppState> = state;
        async move {
            let mut watcher: Option<(PathBuf, FsWatcher)> = None;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)).await;

                // (Re)start the watcher when the workspace changes.
                let ws_root = app.read().workspace.as_ref().map(|w| w.root_path.clone());
                match (&ws_root, watcher.as_ref().map(|(p, _)| p)) {
                    (Some(new), Some(cur)) if new == cur => {} // unchanged
                    (Some(new), _) => {
                        watcher = FsWatcher::start(new).ok().map(|w| (new.clone(), w));
                    }
                    (None, _) => watcher = None,
                }

                // Drain OS filesystem events.
                if let Some((_, ref fw)) = watcher {
                    let events = fw.drain();
                    if !events.is_empty() {
                        let mut s = app.write();
                        let sess_path = s.session.as_ref().map(|sess| sess.path.clone());
                        let mut need_tree_refresh = false;
                        for evt in events {
                            match evt {
                                WatchEvent::Modified(p) | WatchEvent::Deleted(p) => {
                                    if sess_path.as_ref() == Some(&p) {
                                        s.check_external_change();
                                    }
                                    need_tree_refresh = true;
                                }
                                WatchEvent::Created(_) => need_tree_refresh = true,
                            }
                        }
                        if need_tree_refresh {
                            s.refresh_tree();
                        }
                    }
                }

                // Autosave + fallback external-change check (catches renames
                // and platforms where the watcher is unavailable).
                let mut s = app.write();
                if s.session.is_some() {
                    s.check_external_change();
                    let _ = s.autosave_tick(now_ms());
                }
            }
        }
    });

    // Global keyboard shortcuts via eval relay.
    let mut mode_signal = use_context::<Signal<EditorMode>>();
    let mut app_state: Signal<AppState> = state;
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        let relay_js = r#"
            window.__bk_shortcut_relay = (msg) => dioxus.send(msg);
            (async () => { while(true) { await new Promise(r => setTimeout(r, 86_400_000)); } })();
        "#;
        let mut relay = document::eval(relay_js);
        while let Ok(raw) = relay.recv().await {
            if let Ok(AppMsg::Shortcut { key }) = serde_json::from_value(raw) {
                match key.as_str() {
                    "save" => {
                        let _ = app_state.write().save_now(now_ms());
                    }
                    "mode_text" => mode_signal.set(EditorMode::Text),
                    "mode_form" => mode_signal.set(EditorMode::Form),
                    "mode_preview" => mode_signal.set(EditorMode::Preview),
                    "mode_split" => mode_signal.set(EditorMode::Split),
                    _ => {}
                }
            }
        }
    });

    let workspace_open = state.read().workspace.is_some();
    let settings_open = *use_context::<Signal<bool>>().read();

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        document::Script { src: SHORTCUTS_JS }
        ToastLayer {}
        if settings_open {
            SettingsScreen {}
        } else if workspace_open {
            MainShell {}
        } else {
            StartScreen {}
        }
    }
}

fn false_val() -> bool {
    false
}

#[component]
fn MainShell() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mode = *use_context::<Signal<EditorMode>>().read();
    let collapsed = *use_context::<Signal<bool>>().read();
    let outline_open = use_context::<Signal<bool>>();
    let has_document = state.read().session.is_some();

    rsx! {
        div { class: "shell",
            if !collapsed { Explorer {} }
            main { class: "editor-pane",
                EditorHeader {}
                ConflictBanner {}
                div { class: "surface-row",
                    div { class: "surface",
                        if has_document {
                            match mode {
                                EditorMode::Text    => rsx! { TextMode {} },
                                EditorMode::Form    => rsx! { FormMode {} },
                                EditorMode::Preview => rsx! { PreviewMode {} },
                                EditorMode::Split   => rsx! { SplitMode {} },
                            }
                        } else {
                            p { class: "empty-hint", {tr(lang, "editor.no_document")} }
                        }
                    }
                    if *outline_open.read() && has_document {
                        OutlinePanel {}
                    }
                }
                StatusBar {}
            }
        }
    }
}
