//! Root application component.

use crate::bridge;
use std::path::PathBuf;

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_fs::{FsWatcher, WatchEvent};
use bekoedit_ui_contract::EditorMode;

use crate::components::{
    app_bar::AppBar,
    backlinks_panel::BacklinksPanel,
    conflict_banner::ConflictBanner,
    editor_header::EditorHeader,
    explorer::Explorer,
    form_mode::FormMode,
    history_panel::HistoryPanel,
    outline_panel::OutlinePanel,
    preview_mode::PreviewMode,
    recovery_screen::RecoveryScreen,
    search_panel::SearchPanel,
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
    use_context_provider(|| Signal::new(false_val())); // search panel open
    use_context_provider(|| Signal::new(false_val())); // backlinks panel open
    use_context_provider(|| Signal::new(false_val())); // history panel open
    use_context_provider(|| Signal::new(Vec::<Toast>::new()));
    let recovery_dismissed = use_signal(|| false);

    // Background: native fs watcher + autosave + external-change poll.
    use_future(move || {
        let mut app: Signal<AppState> = state;
        async move {
            let mut watcher: Option<(PathBuf, FsWatcher)> = None;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)).await;
                let ws_root = app.read().workspace.as_ref().map(|w| w.root_path.clone());
                match (&ws_root, watcher.as_ref().map(|(p, _)| p)) {
                    (Some(n), Some(c)) if n == c => {}
                    (Some(n), _) => {
                        watcher = FsWatcher::start(n).ok().map(|w| (n.clone(), w));
                    }
                    (None, _) => watcher = None,
                }
                if let Some((_, ref fw)) = watcher {
                    let events = fw.drain();
                    if !events.is_empty() {
                        let mut s = app.write();
                        let sess = s.session.as_ref().map(|ss| ss.path.clone());
                        let mut refresh = false;
                        for ev in events {
                            match ev {
                                WatchEvent::Modified(p) | WatchEvent::Deleted(p) => {
                                    if sess.as_ref() == Some(&p) {
                                        s.check_external_change();
                                    }
                                    refresh = true;
                                }
                                WatchEvent::Created(_) => refresh = true,
                            }
                        }
                        if refresh {
                            s.refresh_tree();
                        }
                    }
                }
                let mut s = app.write();
                if s.session.is_some() {
                    s.check_external_change();
                    let _ = s.autosave_tick(now_ms());
                }
            }
        }
    });

    // Global keyboard shortcut relay.
    let mut mode_sig = use_context::<Signal<EditorMode>>();
    let mut app_st: Signal<AppState> = state;
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        // Auto-restarting shortcut relay (RFC-002 hardening).
        for _ in 0..bridge::MAX_RELAY_RESTARTS {
            let relay_js = bridge::relay_js("__bk_shortcut_relay");
            let mut relay = document::eval(&relay_js);
            while let Ok(raw) = relay.recv().await {
                if let Ok(AppMsg::Shortcut { key }) = serde_json::from_value(raw) {
                    match key.as_str() {
                        "save" => {
                            let _ = app_st.write().save_now(now_ms());
                        }
                        "mode_text" => mode_sig.set(EditorMode::Text),
                        "mode_form" => mode_sig.set(EditorMode::Form),
                        "mode_preview" => mode_sig.set(EditorMode::Preview),
                        "mode_split" => mode_sig.set(EditorMode::Split),
                        _ => {}
                    }
                }
            }
            // relay disconnected — restart after a brief pause
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    let has_recovery = !*recovery_dismissed.read() && has_pending_recovery(&state.read());
    let workspace_open = state.read().workspace.is_some() || state.read().session.is_some();
    let settings_open = *use_context::<Signal<bool>>().read();

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        document::Script { src: SHORTCUTS_JS }
        ToastLayer {}
        AppBar {}
        if settings_open {
            SettingsScreen {}
        } else if has_recovery {
            RecoveryScreen { dismissed: recovery_dismissed }
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

pub(crate) fn has_pending_recovery(state: &AppState) -> bool {
    !state.recovery.list().is_empty()
}

#[component]
fn MainShell() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mode = *use_context::<Signal<EditorMode>>().read();
    let collapsed = *use_context::<Signal<bool>>().read(); // explorer
    let outline_open = use_context::<Signal<bool>>(); // 3rd bool
    let search_open = use_context::<Signal<bool>>(); // 4th
    let backlinks_open = use_context::<Signal<bool>>(); // 5th
    let history_open = use_context::<Signal<bool>>(); // 6th
    let has_doc = state.read().session.is_some();

    rsx! {
        div { class: "shell",
            if !collapsed { Explorer {} }
            main { class: "editor-pane",
                EditorHeader {}
                ConflictBanner {}
                div { class: "surface-row",
                    // Left: main editor surface
                    div { class: "surface",
                        if has_doc {
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
                    // Right panels (mutually exclusive or stacked)
                    if *history_open.read() && has_doc {
                        HistoryPanel {}
                    } else if *search_open.read() {
                        SearchPanel {}
                    } else if *backlinks_open.read() && has_doc {
                        BacklinksPanel {}
                    } else if *outline_open.read() && has_doc {
                        OutlinePanel {}
                    }
                }
                StatusBar {}
            }
        }
    }
}
