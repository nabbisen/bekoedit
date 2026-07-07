//! Inline formatting toolbar component (RFC-030).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, InlineFormat, fingerprint::BlockId};

use super::dispatch;
use crate::i18n::{Lang, tr};

// ─── Inline formatting toolbar (RFC-030) ─────────────────────────────────────

/// Renders a compact B / I / ` / 🔗 toolbar.
/// `field_id` is the DOM id of the associated textarea/input so the JS can
/// read `selectionStart`/`End` after we prevent the textarea from losing
/// focus on mousedown.
#[component]
pub fn InlineToolbar(field_id: String, block_id: BlockId, revision: u64, lang: Lang) -> Element {
    let state = use_context::<Signal<AppState>>();

    let make_btn = |label: &'static str, aria: &'static str, kind: InlineFormat| {
        let fid = field_id.clone();
        rsx! {
            button {
                class: "inline-fmt-btn",
                aria_label: aria,
                title: aria,
                // Prevent textarea from losing focus on mousedown.
                onmousedown: |evt| evt.prevent_default(),
                onclick: move |_| {
                    // Read selection from the textarea, then dispatch the command.
                    let js = format!(
                        r#"
                        (function() {{
                            var el = document.getElementById({id});
                            var s = el ? el.selectionStart : 0;
                            var e = el ? el.selectionEnd   : 0;
                            window.__bk_form_relay?.(JSON.stringify({{s:s,e:e}}));
                        }})();
                        "#,
                        id = serde_json::to_string(&fid).unwrap()
                    );
                    // Receive the selection asynchronously then dispatch.
                    let bid   = block_id;
                    let rev   = revision;
                    let k     = kind;
                    let st = state;
                    spawn(async move {
                        let relay_js = r#"
                            window.__bk_form_relay = (msg) => dioxus.send(msg);
                            (async()=>{ while(true){ await new Promise(r=>setTimeout(r,86400000));} })();
                        "#;
                        let mut relay = document::eval(relay_js);
                        // Fire the selection-read JS, then wait for the reply.
                        document::eval(&js);
                        if let Ok(raw) = relay.recv().await {
                            #[derive(serde::Deserialize)]
                            struct Sel { s: usize, e: usize }
                            if let Ok(Sel { s, e }) = serde_json::from_value::<Sel>(raw) {
                                dispatch(st, rev, bid, FormBlockEdit::ToggleInline {
                                    kind: k,
                                    utf16_start: s,
                                    utf16_len: e.saturating_sub(s),
                                    link_url: None,
                                });
                            }
                        }
                    });
                },
                {label}
            }
        }
    };

    rsx! {
        div { class: "inline-toolbar",
            {make_btn("B", tr(lang, "fmt.bold"),   InlineFormat::Bold)}
            {make_btn("I", tr(lang, "fmt.italic"), InlineFormat::Italic)}
            {make_btn("`", tr(lang, "fmt.code"),   InlineFormat::Code)}
        }
    }
}
