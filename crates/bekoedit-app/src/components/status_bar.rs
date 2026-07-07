//! Status bar — save state only by default (Less is more).
//!
//! A single line at the bottom. The most important thing a user needs
//! to see is whether their file is saved. Word count, line endings, and
//! parse diagnostics are noise until the user is looking for them.
//! They appear as tooltips on the save state indicator.

use dioxus::prelude::*;

use bekoedit_core::{AppState, SaveState};

use crate::i18n::{Lang, tr};

#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let ui_lang = *use_context::<Signal<Lang>>().read();

    let (save_key, is_failure, detail_tip) = {
        let s = state.read();
        let key = s.save_state.label_key();
        let fail = matches!(s.save_state, SaveState::SaveFailed { .. });

        // Compact detail tooltip (shown on hover): word count + line ending
        let tip = s
            .session
            .as_ref()
            .map(|doc| {
                let (words, _) = doc.word_char_count();
                let le = format!("{:?}", doc.line_ending);
                let islands = doc.index.raw_islands.len();
                let diags = doc.index.diagnostics.len();
                let mut parts = vec![format!("{words} words"), le];
                if islands > 0 {
                    parts.push(format!("{islands} island(s)"));
                }
                if diags > 0 {
                    parts.push(format!("{diags} warning(s)"));
                }
                parts.join(" · ")
            })
            .unwrap_or_default();

        (key, fail, tip)
    };

    let role = if is_failure { "alert" } else { "status" };
    let live = if is_failure { "assertive" } else { "polite" };

    rsx! {
        footer { class: "status-bar",
            span {
                role,
                aria_live: live,
                aria_atomic: "true",
                class: "save-state {save_key.replace('.', \"-\")}",
                title: "{detail_tip}",
                {tr(ui_lang, save_key)}
            }
        }
    }
}
