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

use crate::state::now_ms;

const CM_SPLIT_ID: &str = "cm-split";
const PREVIEW_SPLIT_ID: &str = "preview-split";

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SplitMsg {
    Change { revision: u64, text: String },
    Scroll { fraction: f64 },
    Ready,
}

#[component]
pub fn SplitMode() -> Element {
    let mut app_state = use_context::<Signal<AppState>>();

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
        let js = format!(
            "if(window.__bk){{window.__bk.init({},{},{},{});}}",
            serde_json::to_string(CM_SPLIT_ID).unwrap(),
            serde_json::to_string(&text_init).unwrap(),
            doc_id,
            revision,
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
        let js = format!(
            "if(window.__bk){{window.__bk.setDoc({},{},{});}}",
            serde_json::to_string(&text_sync).unwrap(),
            doc_id,
            revision,
        );
        document::eval(&js);
        cm_doc_id.set(doc_id);
        cm_revision.set(revision);
    });

    // Relay: receives CM6 changes AND scroll events.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        let relay_js = r#"
            window.__bk_split_relay = (msg) => dioxus.send(msg);
            // Install scroll listener once CM6 is ready.
            const installScroll = () => {
                const scroller = document.querySelector(`#cm-split .cm-scroller`);
                if (!scroller) { setTimeout(installScroll, 100); return; }
                scroller.addEventListener('scroll', () => {
                    const frac = scroller.scrollHeight > scroller.clientHeight
                        ? scroller.scrollTop / (scroller.scrollHeight - scroller.clientHeight)
                        : 0;
                    window.__bk_split_relay?.(JSON.stringify({type:'scroll', fraction: frac}));
                }, { passive: true });
            };
            setTimeout(installScroll, 200);
            (async () => { while(true) { await new Promise(r => setTimeout(r, 86_400_000)); } })();
        "#;
        let mut relay = document::eval(relay_js);
        while let Ok(raw) = relay.recv().await {
            if let Ok(msg) = serde_json::from_value::<SplitMsg>(raw) {
                match msg {
                    SplitMsg::Change {
                        revision: base,
                        text: new_text,
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
