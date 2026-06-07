//! Text Mode (RFC-011).
//!
//! MVP surface: a plain editing surface bound to the canonical text, using
//! the snapshot synchronization strategy RFC-011 specifies (the editor
//! sends the full text; Rust validates the base revision and reparses).
//! The CodeMirror 6 adapter planned by RFC-011 slots in behind the same
//! `apply_text_snapshot` contract without touching the core.

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::state::now_ms;

#[component]
pub fn TextMode() -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let (text, revision) = {
        let s = state.read();
        let session = s.session.as_ref();
        (
            session
                .map(|s| s.canonical_text.clone())
                .unwrap_or_default(),
            session.map(|s| s.revision).unwrap_or(0),
        )
    };

    rsx! {
        textarea {
            class: "text-mode",
            spellcheck: "false",
            value: "{text}",
            oninput: move |evt| {
                // Snapshot sync: current revision is the base; the store
                // rejects stale snapshots (RFC-011 acceptance).
                let _ = state.write().edit_text(revision, evt.value(), now_ms());
            },
        }
    }
}
