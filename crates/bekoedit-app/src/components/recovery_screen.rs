//! Recovery screen (RFC-007): shown when the app finds snapshots from a
//! previous session that ended before all documents were saved cleanly.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::RecoverySnapshot;

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};

#[component]
pub fn RecoveryScreen(mut dismissed: Signal<bool>) -> Element {
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
                                            match state.write().restore_recovery_snapshot(
                                                &snap,
                                                crate::state::now_ms(),
                                            ) {
                                                Ok(()) => {
                                                    dismissed.set(true);
                                                    push_toast(
                                                        &mut toasts,
                                                        ToastKind::Info,
                                                        tr(lang, "recovery.restored"),
                                                    );
                                                }
                                                Err(err) => push_toast(
                                                    &mut toasts,
                                                    ToastKind::Error,
                                                    err.to_string(),
                                                ),
                                            }
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
                        let s = state.write();
                        for snap in s.recovery.list() {
                            let _ = s.recovery.remove(&snap.original_path);
                        }
                        dismissed.set(true);
                    },
                    {tr(lang, "recovery.skip_all")}
                }
            }
        }
    }
}
