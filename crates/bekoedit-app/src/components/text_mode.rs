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

use crate::state::now_ms;

const EDITOR_BUNDLE: Asset = asset!("/assets/editor-bundle.js");
const CM_CONTAINER: &str = "cm-root";

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum EditorMsg {
    Change {
        // Kept for protocol completeness (RFC-002 §7 document_id field).
        #[allow(dead_code)]
        doc_id: u64,
        revision: u64,
        text: String,
    },
    Ready,
}

#[component]
pub fn TextMode() -> Element {
    let mut app_state = use_context::<Signal<AppState>>();
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
        let js = format!(
            "if(window.__bk){{window.__bk.init({},{},{},{}); }}",
            serde_json::to_string(CM_CONTAINER).unwrap(),
            serde_json::to_string(&text_init).unwrap(),
            doc_id,
            revision,
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
        let js = format!(
            "if(window.__bk){{window.__bk.setDoc({},{},{}); }}",
            serde_json::to_string(&text_sync).unwrap(),
            doc_id,
            revision,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    // Install the relay channel and receive CM6 changes.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        // Bind window.__bk_relay to THIS eval's dioxus.send channel.
        let relay_js = r#"
            window.__bk_relay = (msg) => dioxus.send(msg);
            (async () => {
                while (true) {
                    await new Promise(r => setTimeout(r, 86_400_000));
                }
            })();
        "#;
        let mut relay = document::eval(relay_js);
        while let Ok(raw) = relay.recv().await {
            if let Ok(msg) = serde_json::from_value::<EditorMsg>(raw) {
                match msg {
                    EditorMsg::Change {
                        revision: base,
                        text: new_text,
                        doc_id: _,
                    } => {
                        let _ = app_state.write().edit_text(base, new_text, now_ms());
                        let rev = app_state
                            .read()
                            .session
                            .as_ref()
                            .map(|s| s.revision)
                            .unwrap_or(0);
                        cm_revision.set(rev);
                    }
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
