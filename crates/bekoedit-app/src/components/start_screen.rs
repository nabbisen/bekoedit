//! Start screen (RFC-010): shown when no workspace is open.
//!
//! Provides three entry points:
//! - **Open Folder** — native OS folder picker via `rfd::AsyncFileDialog`
//! - **New File** — blank in-memory document, no workspace required
//! - **Recent workspaces** — reopen a previously used folder

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::components::icons::{FolderIcon, NewFileIcon};
use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::source_sync::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, cancel_source_focus,
    submit_source_interaction,
};
use crate::state::now_ms;
use bekoedit_ui_contract::EditorMode;

#[component]
pub fn StartScreen() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let mode = use_context::<Signal<EditorMode>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let recents: Vec<_> = state
        .read()
        .recents
        .entries
        .iter()
        .map(|e| e.root_path.clone())
        .collect();

    rsx! {
        div { class: "start-screen",
            div { class: "start-card",
                h1 { class: "start-title", {tr(lang, "app.title")} }
                p  { class: "start-tagline muted", {tr(lang, "app.tagline")} }

                div { class: "start-actions",
                    // ── Open Folder ─────────────────────────────────────────
                    button {
                        class: "btn-primary start-btn",
                        aria_label: tr(lang, "start.open_folder"),
                        onclick: move |_| {
                            cancel_source_focus(source_sync);
                            let mut st = state;
                            spawn(async move {
                                if let Some(handle) = rfd::AsyncFileDialog::new()
                                    .set_title("Select a workspace folder")
                                    .pick_folder()
                                    .await
                                {
                                    let _ = st.write().open_workspace(handle.path(), now_ms());
                                }
                            });
                        },
                        FolderIcon {}
                        {tr(lang, "start.open_folder")}
                    }

                    // ── New In-Memory File ───────────────────────────────────
                    button {
                        class: "btn-secondary start-btn",
                        "data-source-focus-launch": "start-new",
                        aria_label: tr(lang, "start.new_file"),
                        onclick: move |_| {
                            submit_source_interaction(
                                source_sync,
                                state,
                                mode,
                                toasts,
                                SourceCommand::NewUntitled,
                                SourceInteractionOrigin::start_control("start-new"),
                                || {},
                            );
                        },
                        NewFileIcon {}
                        {tr(lang, "start.new_file")}
                    }
                }

                // ── Recent workspaces ────────────────────────────────────────
                if !recents.is_empty() {
                    div { class: "start-recents",
                        h2 { class: "start-recents-title", {tr(lang, "start.recents")} }
                        ul { class: "start-recents-list",
                            for path in &recents {
                                li { key: "{path.display()}",
                                    button {
                                        class: "start-recent-btn",
                                        onclick: {
                                            let p = path.clone();
                                            move |_| {
                                                if p.exists() {
                                                    let _ = state.write().open_workspace(&p, now_ms());
                                                }
                                            }
                                        },
                                        span { class: "recent-name",
                                            {path.file_name()
                                                .map(|n| n.to_string_lossy().into_owned())
                                                .unwrap_or_else(|| path.display().to_string())}
                                        }
                                        span { class: "recent-path muted", "{path.display()}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
