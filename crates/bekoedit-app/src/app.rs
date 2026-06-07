//! Root application component (RFC-010 shell, RFC-019 mode switching).
//!
//! Layout: start screen until a workspace is open, then the main shell
//! (explorer sidebar / editor header / active mode surface / status bar).
//! Mode switching never mutates the document: each surface is a projection
//! of the same canonical text (RFC-019).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::{
    conflict_banner::ConflictBanner, editor_header::EditorHeader, explorer::Explorer,
    form_mode::FormMode, preview_mode::PreviewMode, start_screen::StartScreen,
    status_bar::StatusBar, text_mode::TextMode,
};
use crate::i18n::{Lang, tr};
use crate::state::{create_app_state, now_ms};

const STYLE: Asset = asset!("/assets/style.css");

/// Autosave polling cadence; cheap because `autosave_tick` is a no-op
/// until the debounce deadline passes.
const TICK_MS: u64 = 500;

#[component]
pub fn App() -> Element {
    let state = use_context_provider(|| Signal::new(create_app_state()));
    use_context_provider(|| Signal::new(Lang::default()));
    use_context_provider(|| Signal::new(EditorMode::Form));

    // Background autosave + external-change polling (RFC-007/008). The
    // interval also drives conflict detection so external edits surface
    // without waiting for a save attempt.
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

    let workspace_open = state.read().workspace.is_some();

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        if workspace_open {
            MainShell {}
        } else {
            StartScreen {}
        }
    }
}

#[component]
fn MainShell() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mode = *use_context::<Signal<EditorMode>>().read();
    let has_document = state.read().session.is_some();

    rsx! {
        div { class: "shell",
            Explorer {}
            main { class: "editor-pane",
                EditorHeader {}
                ConflictBanner {}
                div { class: "surface",
                    if has_document {
                        match mode {
                            EditorMode::Text => rsx! { TextMode {} },
                            EditorMode::Form => rsx! { FormMode {} },
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
