//! Root application component (RFC-010/019/020/022).

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::{
    conflict_banner::ConflictBanner,
    editor_header::EditorHeader,
    explorer::Explorer,
    form_mode::FormMode,
    preview_mode::PreviewMode,
    settings_screen::SettingsScreen,
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
    use_context_provider(|| Signal::new(Vec::<Toast>::new()));

    // Background autosave + external-change poll.
    use_future(move || {
        let mut state: Signal<AppState> = state;
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)).await;
                let mut s = state.write();
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
    let has_document = state.read().session.is_some();

    rsx! {
        div { class: "shell",
            if !collapsed { Explorer {} }
            main { class: "editor-pane",
                EditorHeader {}
                ConflictBanner {}
                div { class: "surface",
                    if has_document {
                        match mode {
                            EditorMode::Text    => rsx! { TextMode {} },
                            EditorMode::Form    => rsx! { FormMode {} },
                            EditorMode::Preview => rsx! { PreviewMode {} },
                        }
                    } else {
                        p { class: "empty-hint", {tr(lang, "editor.no_document")} }
                    }
                }
                StatusBar {}
            }
        }
    }
}
