//! Recovery screen (RFC-007): shown when the app finds snapshots from a
//! previous session that ended before all documents were saved cleanly.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::RecoverySnapshot;

use crate::components::toast::{ToastKind, push_toast};
use crate::i18n::{Lang, tr};
use crate::shell_focus;
use crate::source_sync::{SourceSyncState, cancel_source_focus};

const RECOVERY_HEADING: &str = "recovery-heading";

#[component]
pub fn RecoveryScreen(mut dismissed: Signal<bool>) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut toasts = use_context::<Signal<Vec<crate::components::toast::Toast>>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();

    // Screen replacement (RFC-042 §6.3): acquires shell focus authority and
    // moves focus to the heading on entry, once — this effect reads no
    // signal, so Dioxus does not re-run it on later re-renders (matching
    // the same no-read shape `search_panel.rs`'s entry-focus effect uses).
    use_effect(move || {
        cancel_source_focus(source_sync);
        shell_focus::focus_element(RECOVERY_HEADING);
    });
    // Releases shell focus authority and restores focus on unmount — fires
    // for every exit path (restore, per-item discard that empties the
    // list, skip-all) because all three flow through the same `dismissed`
    // signal flip, which is what actually unmounts this component. Recovery
    // is entered at launch, not from a control (handoff §5.2), so there is
    // no invoking trigger to restore to; the app bar's logo is the one
    // control guaranteed present in whatever screen replaces this one
    // (MainShell or StartScreen), standing in for "the next screen's
    // natural first control" rather than an invented phantom trigger.
    use_drop(move || {
        let mut sync = source_sync;
        sync.write().release_shell_focus();
        shell_focus::focus_element(shell_focus::TRIGGER_APP_LOGO);
    });

    let snapshots: Vec<RecoverySnapshot> = state.read().recovery.list();

    rsx! {
        div { class: "recovery-screen",
            div {
                class: "recovery-card",
                role: "region",
                aria_labelledby: RECOVERY_HEADING,
                h2 { id: RECOVERY_HEADING, tabindex: "-1", {tr(lang, "recovery.title")} }
                p { class: "recovery-desc", {tr(lang, "recovery.description")} }
                p {
                    class: "recovery-count",
                    role: "status",
                    "{snapshots.len()} {tr(lang, \"recovery.recoverable_suffix\")}"
                }

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
                                            if state.read().recovery.list().is_empty() {
                                                dismissed.set(true);
                                            }
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
