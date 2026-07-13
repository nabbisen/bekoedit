//! Local document history panel.
//!
//! Shows the last saved versions of the current document, newest first.
//! Clicking "Restore" loads the snapshot as a new dirty edit without
//! writing to disk. The user must explicitly save to commit the restore.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::HistoryEntry;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::source_sync::{SourceCommand, SourceSyncState, submit_source_command};

fn format_time(secs: u64) -> String {
    // Simple formatting: show relative time for recent entries.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let age = now.saturating_sub(secs);
    if age < 60 {
        "just now".into()
    } else if age < 3600 {
        format!("{}m ago", age / 60)
    } else if age < 86400 {
        format!("{}h ago", age / 3600)
    } else {
        format!("{}d ago", age / 86400)
    }
}

#[component]
pub fn HistoryPanel() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let toasts = use_context::<Signal<Vec<Toast>>>();

    let entries: Vec<HistoryEntry> = state.read().list_history();

    rsx! {
        aside {
            class: "outline-panel",
            role: "complementary",
            aria_label: tr(lang, "history.label"),
            h2 { class: "outline-title", {tr(lang, "history.title")} }
            if entries.is_empty() {
                p { class: "muted", {tr(lang, "history.empty")} }
            } else {
                ul { class: "outline-list history-list",
                    for entry in entries {
                        li { class: "history-entry",
                            key: "{entry.saved_at_secs}-{entry.revision}",
                            div { class: "history-meta",
                                span { class: "history-time",
                                    {format_time(entry.saved_at_secs)}
                                }
                                span { class: "muted history-rev",
                                    "rev {entry.revision}"
                                }
                            }
                            button {
                                class: "history-restore",
                                onclick: {
                                    let e = entry.clone();
                                    move |_| {
                                        submit_source_command(
                                            source_sync,
                                            state,
                                            mode_sig,
                                            toasts,
                                            SourceCommand::RestoreHistory(e.clone()),
                                        );
                                    }
                                },
                                {tr(lang, "history.restore")}
                            }
                        }
                    }
                }
            }
        }
    }
}
