//! Text Mode source-editor surface owned by the application-root controller.

use bekoedit_core::AppState;
use bekoedit_ui_contract::source_editor::SourceEditorId;
use dioxus::prelude::*;

use super::source_editor_host::{SourceEditorStatus, use_source_editor_lifecycle};
use crate::{
    i18n::{Lang, tr},
    source_sync::{SourceSyncState, mount_source_editor},
};

const CM_CONTAINER: &str = "cm-root";

#[component]
pub fn TextMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let sync = use_context::<Signal<SourceSyncState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let status = use_source_editor_lifecycle(SourceEditorId::Text);
    rsx! {
        div { class: "source-editor-host",
            div { id: CM_CONTAINER, class: "text-mode-cm" }
            match status {
                SourceEditorStatus::Loading => rsx! {
                    p { class: "empty-hint source-editor-status", {tr(lang, "editor.loading")} }
                },
                SourceEditorStatus::Unavailable => rsx! {
                    div { class: "empty-hint source-editor-status unavailable",
                        span { {tr(lang, "editor.unavailable")} }
                        button {
                            onclick: move |_| {
                                if let Some(session) = state.read().session.as_ref() {
                                    mount_source_editor(
                                        sync,
                                        SourceEditorId::Text,
                                        session.document_id,
                                        session.revision,
                                    );
                                }
                            },
                            {tr(lang, "editor.retry")}
                        }
                    }
                },
                SourceEditorStatus::Ready => rsx! {},
            }
        }
    }
}
