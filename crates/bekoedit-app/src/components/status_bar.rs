//! Status bar (RFC-023): save lifecycle state and diagnostics count.
//!
//! Labels come from `SaveState::label_key`, resolved through i18n.

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};

#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let (label_key, diagnostics, islands) = {
        let s = state.read();
        (
            s.save_state.label_key(),
            s.session
                .as_ref()
                .map(|d| d.index.diagnostics.len())
                .unwrap_or(0),
            s.session
                .as_ref()
                .map(|d| d.index.raw_islands.len())
                .unwrap_or(0),
        )
    };

    rsx! {
        footer { class: "status-bar",
            span { class: "save-state {label_key.replace('.', \"-\")}", {tr(lang, label_key)} }
            if islands > 0 {
                span { class: "muted", "⬚ {islands}" }
            }
            if diagnostics > 0 {
                span { class: "muted", "⚠ {diagnostics}" }
            }
        }
    }
}
