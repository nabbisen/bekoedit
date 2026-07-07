//! Status bar (RFC-023/RFC-021): save lifecycle, word/char count,
//! diagnostic count, and line-ending indicator.

use dioxus::prelude::*;

use bekoedit_core::{AppState, SaveState};

use crate::i18n::{Lang, tr};

#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let (label_key, is_failure, diagnostics, islands, line_ending, words, chars) = {
        let s = state.read();
        let key = s.save_state.label_key();
        let fail = matches!(s.save_state, SaveState::SaveFailed { .. });
        let diag = s
            .session
            .as_ref()
            .map(|d| d.index.diagnostics.len())
            .unwrap_or(0);
        let isl = s
            .session
            .as_ref()
            .map(|d| d.index.raw_islands.len())
            .unwrap_or(0);
        let le = s.session.as_ref().map(|d| format!("{:?}", d.line_ending));
        let (w, c) = s
            .session
            .as_ref()
            .map(|d| d.word_char_count())
            .unwrap_or((0, 0));
        (key, fail, diag, isl, le, w, c)
    };

    let role = if is_failure { "alert" } else { "status" };
    let live = if is_failure { "assertive" } else { "polite" };

    rsx! {
        footer { class: "status-bar",
            span {
                role,
                aria_live: live,
                aria_atomic: "true",
                class: "save-state {label_key.replace('.', \"-\")}",
                {tr(lang, label_key)}
            }
            if words > 0 {
                span {
                    class: "muted",
                    title: "{chars} {tr(lang, \"status.chars\")}",
                    "{words} {tr(lang, \"status.words\")}"
                }
            }
            if let Some(le) = line_ending {
                span { class: "muted", "{le}" }
            }
            if islands > 0 {
                span { class: "muted", title: tr(lang, "status.islands_hint"), "⬚ {islands}" }
            }
            if diagnostics > 0 {
                span { class: "muted", title: tr(lang, "status.diag_hint"), "⚠ {diagnostics}" }
            }
        }
    }
}
