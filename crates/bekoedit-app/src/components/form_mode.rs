//! Form Mode surface (RFC-016/017/018/027/028/030).
//!
//! Renders each block from `FormProjection` as a typed form control.
//! New in v0.4.0:
//! - **Inline formatting toolbar** (RFC-030): B / I / ` / 🔗 buttons above
//!   paragraph/heading/blockquote fields; uses `onmousedown preventDefault`
//!   to keep the textarea focus, then reads `selectionStart/End` via eval.
//! - **Table grid** (RFC-027): simple GFM tables as editable cell grids.
//! - **Image card** (RFC-028): image preview + editable alt/src fields.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockEdit, FormEditCommand, FormProjection, fingerprint::BlockId};

use crate::i18n::Lang;
use crate::state::now_ms;

#[component]
pub fn FormMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let projection: Option<FormProjection> =
        state.read().session.as_ref().map(|s| s.form_projection());
    let Some(projection) = projection else {
        return rsx! { div {} };
    };
    let revision = projection.document_revision;

    rsx! {
        div { class: "form-mode",
            for block in projection.blocks {
                FormBlockView {
                    key: "{block.block_id.ordinal}-{block.block_id.fingerprint.content_hash}",
                    block_id: block.block_id,
                    display: block.display.clone(),
                    revision,
                    lang,
                }
            }
        }
    }
}

fn dispatch(mut state: Signal<AppState>, revision: u64, block_id: BlockId, edit: FormBlockEdit) {
    let cmd = FormEditCommand {
        base_revision: revision,
        block_id,
        client_block_fingerprint: Some(block_id.fingerprint),
        edit,
    };
    let _ = state.write().edit_form(&cmd, now_ms());
}

mod block_view;
mod inline_toolbar;

use block_view::FormBlockView;
