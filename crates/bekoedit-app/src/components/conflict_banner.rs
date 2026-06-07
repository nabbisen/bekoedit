//! Conflict resolution banner (RFC-008, external design §19.4).
//!
//! Shown when the open document changed (or disappeared) on disk while
//! memory holds unsaved edits. Resolution is always an explicit choice;
//! Save Copy proposes a non-colliding sibling name.

use dioxus::prelude::*;

use bekoedit_core::{AppState, ConflictResolution, ConflictState};

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn ConflictBanner() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let conflict = state.read().conflict;
    if !conflict.requires_user_decision() {
        return rsx! {};
    }
    let title_key = if conflict == ConflictState::FileDeletedOnDisk {
        "conflict.deleted"
    } else {
        "conflict.title"
    };
    let copy_name = state
        .read()
        .session
        .as_ref()
        .and_then(|s| s.path.file_stem().map(|n| n.to_string_lossy().into_owned()))
        .map(|stem| format!("{stem}-conflict-copy.md"))
        .unwrap_or_else(|| "conflict-copy.md".into());

    let mut resolve = move |resolution: ConflictResolution| {
        let _ = state.write().resolve_conflict(resolution, now_ms());
    };

    rsx! {
        div { class: "conflict-banner",
            p { {tr(lang, title_key)} }
            div { class: "conflict-actions",
                button { onclick: move |_| resolve(ConflictResolution::KeepMine),
                    {tr(lang, "conflict.keep_mine")}
                }
                if conflict != ConflictState::FileDeletedOnDisk {
                    button { onclick: move |_| resolve(ConflictResolution::ReloadDisk),
                        {tr(lang, "conflict.reload")}
                    }
                }
                button {
                    onclick: move |_| resolve(ConflictResolution::SaveCopy {
                        relative_path: copy_name.clone().into(),
                    }),
                    {tr(lang, "conflict.save_copy")}
                }
            }
        }
    }
}
