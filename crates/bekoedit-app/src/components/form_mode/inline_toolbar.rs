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
///
/// `cell`, when `Some((row, col))`, targets one table cell (RFC-048 slice
/// 2 §2.3) instead of the block's own single content range: the same
/// toolbar, acting on whichever cell last took focus, dispatches
/// `ToggleInlineInTableCell` rather than `ToggleInline`. Offsets are still
/// read the same way, since both are UTF-16 code units relative to
/// whatever `field_id` names -- only the target of the dispatched edit
/// differs.
///
/// `enabled` (review, 2026-10-02 §3.2): a table's toolbar is rendered at
/// all times, not only once a cell has focus, so the table does not shift
/// down by the toolbar's own height under the user's pointer on their
/// first click into it. While no cell has focus the buttons are present
/// but `disabled`, reserving the space without doing anything.
///
/// A click reads the selection through `crate::bridge::eval_body` (task 047
/// Part B): one bounded, self-releasing round trip per click, exactly the
/// shape `release_checks::dom`'s one-shot reads already used. Before this,
/// every click installed a brand-new window-bound relay function plus its
/// own never-ending keep-alive loop, rebinding the one over the last
/// without ever releasing it -- each click left its predecessor's loop and
/// query running forever, accumulating over the session. A one-shot read
/// has nothing left to accumulate: its query is released, and its
/// page-side promise resolves, within the same round trip that answers
/// the click.
#[component]
pub fn InlineToolbar(
    field_id: String,
    block_id: BlockId,
    revision: u64,
    lang: Lang,
    #[props(default)] cell: Option<(usize, usize)>,
    #[props(default = true)] enabled: bool,
) -> Element {
    let state = use_context::<Signal<AppState>>();

    let make_btn = |label: &'static str, aria: &'static str, kind: InlineFormat| {
        let fid = field_id.clone();
        rsx! {
            button {
                class: "inline-fmt-btn",
                aria_label: aria,
                title: aria,
                disabled: !enabled,
                // Prevent textarea from losing focus on mousedown.
                onmousedown: |evt| evt.prevent_default(),
                onclick: move |_| {
                    // Read the textarea's selection in one bounded,
                    // self-releasing round trip (task 047 Part B), then
                    // dispatch the command.
                    let js = format!(
                        "const el = document.getElementById({id}); \
                         return {{ s: el ? el.selectionStart : 0, e: el ? el.selectionEnd : 0 }};",
                        id = crate::bridge::js_string_literal(&fid)
                    );
                    let bid = block_id;
                    let rev = revision;
                    let k = kind;
                    let st = state;
                    spawn(async move {
                        #[derive(serde::Deserialize)]
                        struct Sel { s: usize, e: usize }
                        if let Ok(Sel { s, e }) = crate::bridge::eval_body::<Sel>(&js).await {
                            let utf16_start = s;
                            let utf16_len = e.saturating_sub(s);
                            let edit = match cell {
                                Some((row, col)) => FormBlockEdit::ToggleInlineInTableCell {
                                    row,
                                    col,
                                    kind: k,
                                    utf16_start,
                                    utf16_len,
                                    link_url: None,
                                },
                                None => FormBlockEdit::ToggleInline {
                                    kind: k,
                                    utf16_start,
                                    utf16_len,
                                    link_url: None,
                                },
                            };
                            dispatch(st, rev, bid, edit);
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
