//! Recovery screen (RFC-007): shown when the app finds snapshots from a
//! previous session that ended before all documents were saved cleanly.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::RecoverySnapshot;

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};

#[component]
pub fn RecoveryScreen() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();

    let snapshots: Vec<RecoverySnapshot> = state.read().recovery.list();

    rsx! {
        div { class: "recovery-screen",
            div { class: "recovery-card",
                h2 { {tr(lang, "recovery.title")} }
                p { class: "recovery-desc", {tr(lang, "recovery.description")} }

                ul { class: "recovery-list",
                    for snap in &snapshots {
                        li { class: "recovery-item",
                            key: "{snap.original_path.display()}",
                            div { class: "recovery-path",
                                "{snap.original_path.display()}"
                            }
                            div { class: "recovery-meta muted",
                                "rev {snap.revision}"
                            }
                            div { class: "recovery-actions",
                                button {
                                    class: "btn-primary",
                                    onclick: {
                                        let snap = snap.clone();
                                        move |_| {
                                            let mut s = state.write();
                                            if let Some(ws) = s.workspace.as_ref().map(|w| w.root_path.clone())
                                                && let Ok(rel) = snap.original_path.strip_prefix(&ws) {
                                                let _ = s.open_document(rel);
                                            }
                                            let _ = s.restore_history(&bekoedit_fs::HistoryEntry {
                                                original_path: snap.original_path.clone(),
                                                text: snap.text.clone(),
                                                saved_at_secs: snap.created_at_secs,
                                                revision: snap.revision,
                                            });
                                            let _ = s.recovery.remove(&snap.original_path);
                                            push_toast(&mut toasts, ToastKind::Info,
                                                tr(lang, "recovery.restored"));
                                        }
                                    },
                                    {tr(lang, "recovery.restore")}
                                }
                                button {
                                    class: "btn-ghost",
                                    onclick: {
                                        let path = snap.original_path.clone();
                                        move |_| {
                                            let _ = state.write().recovery.remove(&path);
                                        }
                                    },
                                    {tr(lang, "recovery.discard")}
                                }
                            }
                        }
                    }
                }

                button {
                    class: "btn-ghost recovery-skip",
                    onclick: move |_| {
                        // Dismiss without acting — user can recover later via History panel
                        let s = state.write();
                        for snap in s.recovery.list() {
                            let _ = s.recovery.remove(&snap.original_path);
                        }
                    },
                    {tr(lang, "recovery.skip_all")}
                }
            }
        }
    }
}
