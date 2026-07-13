//! Text Mode (RFC-011): CodeMirror 6 editor with bidirectional Dioxus bridge.
//!
//! The pre-built bundle (assets/editor-bundle.js) exposes window.__bk.
//! A coroutine installs window.__bk_relay (bound to its eval channel) so
//! CM6's debounced change events route back to Rust without touching the
//! generic IPC. Rust pushes setDoc only when the revision jumps from an
//! external source (reload, form edit, new file).

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::bridge;
use crate::source_sync::{
    EditorSnapshot, SnapshotBlockReason, SnapshotBlocked, SourceEditorId, SourceSyncState,
    handle_editor_snapshot, handle_snapshot_blocked,
};

const EDITOR_BUNDLE: Asset = asset!("/assets/editor-bundle.js");
const CM_CONTAINER: &str = "cm-root";
const TEXT_RELAY: &str = "__bk_text_relay";

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum EditorMsg {
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
        #[allow(dead_code)]
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
pub fn TextMode() -> Element {
    let app_state = use_context::<Signal<AppState>>();
    let mut source_sync = use_context::<Signal<SourceSyncState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();
    let mut cm_doc_id = use_signal(|| 0u64);
    let mut cm_revision = use_signal(|| 0u64);

    let (text, doc_id, revision) = {
        let s = app_state.read();
        match s.session.as_ref() {
            Some(sess) => (sess.canonical_text.clone(), sess.document_id, sess.revision),
            None => (String::new(), 0, 0),
        }
    };

    // (Re)initialise CM6 when the document identity changes.
    let text_init = text.clone();
    use_effect(move || {
        let active = source_sync.write().register_editor(
            SourceEditorId::Text,
            EditorMode::Text,
            doc_id,
            revision,
        );
        let js = format!(
            "if(window.__bk){{window.__bk.init({},{},{},{},{},{},{}); }}",
            serde_json::to_string(CM_CONTAINER).unwrap(),
            serde_json::to_string(&text_init).unwrap(),
            doc_id,
            revision,
            serde_json::to_string(SourceEditorId::Text.as_str()).unwrap(),
            serde_json::to_string(TEXT_RELAY).unwrap(),
            active.epoch,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    // Push external changes into CM6 when revision jumped from outside.
    let text_sync = text.clone();
    use_effect(move || {
        let stored = revision;
        let local = *cm_revision.read();
        let same = *cm_doc_id.read() == doc_id;
        if same && stored <= local + 1 {
            return;
        }
        let active = source_sync.write().register_editor(
            SourceEditorId::Text,
            EditorMode::Text,
            doc_id,
            revision,
        );
        let js = format!(
            "if(window.__bk){{window.__bk.setDoc({},{},{},{}); }}",
            serde_json::to_string(&text_sync).unwrap(),
            doc_id,
            revision,
            active.epoch,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    // Same-document protected commands such as History restore and Outline
    // moves mutate canonical Rust state while Text Mode remains mounted. Those
    // must force CodeMirror to refresh even when the ordinary revision guard
    // would otherwise skip.
    let text_refresh = text.clone();
    use_effect(move || {
        let request = source_sync.read().pending_refresh_for(SourceEditorId::Text);
        if let Some(request) = request {
            let js = format!(
                "if(window.__bk){{window.__bk.setDoc({},{},{},{}); }}",
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
                .clear_refresh(SourceEditorId::Text, request.epoch);
        }
    });

    // If a protected source command is waiting, ask this mounted editor for an
    // exact snapshot. Mode switch/save/open happens only after the response is
    // accepted by Rust.
    use_effect(move || {
        let request = source_sync.read().unsent_request_for(SourceEditorId::Text);
        if let Some(request) = request {
            let js = format!(
                "if(window.__bk){{window.__bk.requestSnapshot({},{},{},{}); }}",
                request.request_id,
                serde_json::to_string(SourceEditorId::Text.as_str()).unwrap(),
                request.document_id,
                request.epoch,
            );
            document::eval(&js);
            source_sync.write().mark_request_sent(request.request_id);
        }
    });

    // Install the relay channel and receive CM6 changes.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        let relay_js = bridge::relay_js(TEXT_RELAY);
        let mut relay = document::eval(&relay_js);
        while let Ok(raw) = relay.recv().await {
            if let Ok(msg) = serde_json::from_value::<EditorMsg>(raw) {
                match msg {
                    EditorMsg::Change {
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
                    EditorMsg::Snapshot {
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
                    EditorMsg::SnapshotBlocked {
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
                    EditorMsg::Scroll { fraction: _ } => {}
                    EditorMsg::Ready => {}
                }
            }
        }
    });

    rsx! {
        document::Script { src: EDITOR_BUNDLE }
        div {
            id: CM_CONTAINER,
            class: "text-mode-cm",
            role: "textbox",
            aria_label: "Markdown source editor",
            aria_multiline: "true",
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
