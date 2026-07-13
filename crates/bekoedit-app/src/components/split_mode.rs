//! Split Mode (RFC-010/012): Text editor on the left, sanitized preview on
//! the right, with proportional scroll synchronisation.
//!
//! Scroll-sync mechanism (RFC-012):
//! - A one-time `eval` installs a scroll listener on the CM6 scroll container.
//! - On scroll, the listener computes the fractional scroll position
//!   (scrollTop / scrollHeight) and sends it via the __bk_split_relay.
//! - A Rust coroutine receives the fraction and mirrors it to the preview
//!   div via `eval`.

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::bridge;
use crate::source_sync::{
    EditorSnapshot, SnapshotBlockReason, SnapshotBlocked, SourceEditorId, SourceSyncState,
    handle_editor_snapshot, handle_snapshot_blocked,
};

const CM_SPLIT_ID: &str = "cm-split";
const PREVIEW_SPLIT_ID: &str = "preview-split";
const SPLIT_RELAY: &str = "__bk_split_relay";

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SplitMsg {
    Change {
        #[serde(rename = "editorId")]
        editor_id: String,
        #[serde(rename = "docId")]
        doc_id: u64,
        epoch: u64,
        seq: u64,
        text: String,
        composing: bool,
    },
    Snapshot {
        request_id: u64,
        #[serde(rename = "editorId")]
        editor_id: String,
        #[serde(rename = "docId")]
        doc_id: u64,
        epoch: u64,
        seq: u64,
        text: String,
        composing: bool,
    },
    SnapshotBlocked {
        request_id: u64,
        #[serde(rename = "editorId")]
        editor_id: String,
        #[serde(rename = "docId")]
        doc_id: u64,
        epoch: u64,
        reason: SnapshotBlockReasonWire,
    },
    Scroll {
        fraction: f64,
    },
    Ready,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SnapshotBlockReasonWire {
    CompositionActive,
    EditorUnavailable,
    IdentityMismatch,
    BridgeError,
}

#[component]
pub fn SplitMode() -> Element {
    let app_state = use_context::<Signal<AppState>>();
    let mut source_sync = use_context::<Signal<SourceSyncState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();

    let mut cm_revision = use_signal(|| 0u64);
    let mut cm_doc_id = use_signal(|| 0u64);

    let (text, doc_id, revision, html) = {
        let s = app_state.read();
        match s.session.as_ref() {
            Some(sess) => (
                sess.canonical_text.clone(),
                sess.document_id,
                sess.revision,
                sess.preview_html(),
            ),
            None => (String::new(), 0, 0, String::new()),
        }
    };

    // Init CM6 in the split container.
    let text_init = text.clone();
    use_effect(move || {
        let active = source_sync.write().register_editor(
            SourceEditorId::Split,
            EditorMode::Split,
            doc_id,
            revision,
        );
        let js = format!(
            "if(window.__bk){{window.__bk.init({},{},{},{},{},{},{});}}",
            serde_json::to_string(CM_SPLIT_ID).unwrap(),
            serde_json::to_string(&text_init).unwrap(),
            doc_id,
            revision,
            serde_json::to_string(SourceEditorId::Split.as_str()).unwrap(),
            serde_json::to_string(SPLIT_RELAY).unwrap(),
            active.epoch,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    // Push external revision changes into CM6.
    let text_sync = text.clone();
    use_effect(move || {
        if *cm_doc_id.read() == doc_id && revision <= *cm_revision.read() + 1 {
            return;
        }
        let active = source_sync.write().register_editor(
            SourceEditorId::Split,
            EditorMode::Split,
            doc_id,
            revision,
        );
        let js = format!(
            "if(window.__bk){{window.__bk.setDoc({},{},{},{});}}",
            serde_json::to_string(&text_sync).unwrap(),
            doc_id,
            revision,
            active.epoch,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    let text_refresh = text.clone();
    use_effect(move || {
        let request = source_sync
            .read()
            .pending_refresh_for(SourceEditorId::Split);
        if let Some(request) = request {
            let js = format!(
                "if(window.__bk){{window.__bk.setDoc({},{},{},{});}}",
                serde_json::to_string(&text_refresh).unwrap(),
                request.document_id,
                request.revision,
                request.epoch,
            );
            document::eval(&js);
            cm_doc_id.set(request.document_id);
            cm_revision.set(request.revision);
            source_sync
                .write()
                .clear_refresh(SourceEditorId::Split, request.epoch);
        }
    });

    use_effect(move || {
        let request = source_sync.read().unsent_request_for(SourceEditorId::Split);
        if let Some(request) = request {
            let js = format!(
                "if(window.__bk){{window.__bk.requestSnapshot({},{},{},{});}}",
                request.request_id,
                serde_json::to_string(SourceEditorId::Split.as_str()).unwrap(),
                request.document_id,
                request.epoch,
            );
            document::eval(&js);
            source_sync.write().mark_request_sent(request.request_id);
        }
    });

    // Relay: receives CM6 changes AND scroll events.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        let relay_js = bridge::relay_js(SPLIT_RELAY);
        let mut relay = document::eval(&relay_js);
        while let Ok(raw) = relay.recv().await {
            if let Ok(msg) = serde_json::from_value::<SplitMsg>(raw) {
                match msg {
                    SplitMsg::Change {
                        editor_id,
                        doc_id,
                        epoch,
                        seq,
                        text: new_text,
                        composing,
                    } => {
                        handle_editor_snapshot(
                            source_sync,
                            app_state,
                            mode_sig,
                            toasts,
                            EditorSnapshot {
                                request_id: None,
                                editor_id: parse_editor_id(&editor_id),
                                document_id: doc_id,
                                epoch,
                                seq,
                                text: new_text,
                                composing,
                            },
                        );
                        let rev = app_state
                            .read()
                            .session
                            .as_ref()
                            .map(|s| s.revision)
                            .unwrap_or(0);
                        cm_revision.set(rev);
                    }
                    SplitMsg::Snapshot {
                        request_id,
                        editor_id,
                        doc_id,
                        epoch,
                        seq,
                        text,
                        composing,
                    } => {
                        handle_editor_snapshot(
                            source_sync,
                            app_state,
                            mode_sig,
                            toasts,
                            EditorSnapshot {
                                request_id: Some(request_id),
                                editor_id: parse_editor_id(&editor_id),
                                document_id: doc_id,
                                epoch,
                                seq,
                                text,
                                composing,
                            },
                        );
                        let rev = app_state
                            .read()
                            .session
                            .as_ref()
                            .map(|s| s.revision)
                            .unwrap_or(0);
                        cm_revision.set(rev);
                    }
                    SplitMsg::SnapshotBlocked {
                        request_id,
                        editor_id,
                        doc_id,
                        epoch,
                        reason,
                    } => handle_snapshot_blocked(
                        source_sync,
                        toasts,
                        SnapshotBlocked {
                            request_id,
                            editor_id: parse_editor_id(&editor_id),
                            document_id: doc_id,
                            epoch,
                            reason: reason.into(),
                        },
                    ),
                    SplitMsg::Scroll { fraction } => {
                        // Mirror fractional scroll position to the preview pane.
                        let js = format!(
                            r#"
                            const p = document.getElementById({id});
                            if (p) {{
                                const max = p.scrollHeight - p.clientHeight;
                                p.scrollTop = max * {frac};
                            }}
                            "#,
                            id = serde_json::to_string(PREVIEW_SPLIT_ID).unwrap(),
                            frac = fraction,
                        );
                        document::eval(&js);
                    }
                    SplitMsg::Ready => {}
                }
            }
        }
    });

    rsx! {
        div { class: "split-mode",
            div { class: "split-left",
                // CM6 container for split mode
                div {
                    id: CM_SPLIT_ID,
                    class: "text-mode-cm",
                    role: "textbox",
                    aria_label: "Markdown source editor",
                    aria_multiline: "true",
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

fn parse_editor_id(value: &str) -> SourceEditorId {
    match value {
        "split" => SourceEditorId::Split,
        _ => SourceEditorId::Text,
    }
}

impl From<SnapshotBlockReasonWire> for SnapshotBlockReason {
    fn from(value: SnapshotBlockReasonWire) -> Self {
        match value {
            SnapshotBlockReasonWire::CompositionActive => Self::CompositionActive,
            SnapshotBlockReasonWire::EditorUnavailable => Self::EditorUnavailable,
            SnapshotBlockReasonWire::IdentityMismatch => Self::IdentityMismatch,
            SnapshotBlockReasonWire::BridgeError => Self::BridgeError,
        }
    }
}
