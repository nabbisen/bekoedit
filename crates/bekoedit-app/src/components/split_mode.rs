//! Split Mode source editor plus sanitized preview.

use bekoedit_core::AppState;
use bekoedit_ui_contract::source_editor::SourceEditorId;
use dioxus::prelude::*;

use super::source_editor_host::{SourceEditorStatus, use_source_editor_lifecycle};
use crate::{
    i18n::{Lang, tr},
    source_sync::{SourceSyncState, mount_source_editor},
};

const CM_SPLIT_ID: &str = "cm-split";
const PREVIEW_SPLIT_ID: &str = "preview-split";

#[component]
pub fn SplitMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let sync = use_context::<Signal<SourceSyncState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let status = use_source_editor_lifecycle(SourceEditorId::Split);
    let html = state
        .read()
        .session
        .as_ref()
        .map(|session| session.preview_html())
        .unwrap_or_default();

    rsx! {
        div { class: "split-mode",
            div { class: "split-left source-editor-host",
                div { id: CM_SPLIT_ID, class: "text-mode-cm" }
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
                                            SourceEditorId::Split,
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
            div { class: "split-divider", aria_hidden: "true" }
            div {
                id: PREVIEW_SPLIT_ID,
                class: "split-right preview",
                dangerous_inner_html: "{html}",
            }
        }
    }
}
