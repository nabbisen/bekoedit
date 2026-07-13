//! Workspace search panel (RFC-033).
//!
//! Runs a synchronous grep-style search across all Markdown files under the
//! workspace root and presents ranked results (exact-case first). For very
//! large workspaces the search is deferred to a `spawn` task so the UI stays
//! responsive.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::{SearchMatch, search_workspace};
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::source_sync::{SourceCommand, SourceSyncState, submit_source_command};

#[component]
pub fn SearchPanel() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut query = use_signal(String::new);
    let results: Signal<Vec<SearchMatch>> = use_signal(Vec::new);
    let running = use_signal(|| false);

    let root = state.read().workspace.as_ref().map(|w| w.root_path.clone());

    rsx! {
        aside {
            class: "search-panel",
            role: "search",
            aria_label: tr(lang, "search.label"),

            h2 { class: "outline-title", {tr(lang, "search.title")} }

            div { class: "search-row",
                input {
                    r#type: "search",
                    class: "search-input",
                    placeholder: tr(lang, "search.placeholder"),
                    aria_label: tr(lang, "search.placeholder"),
                    value: "{query}",
                    oninput: move |evt| query.set(evt.value()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter && let Some(ref r) = root {
                                let q   = query.read().clone();
                                let ro  = r.clone();
                                let mut res = results;
                                let mut run = running;
                                run.set(true);
                                spawn(async move {
                                    let matches = search_workspace(&ro, &q, 200);
                                    res.set(matches);
                                    run.set(false);
                                });
                        }
                    },
                }
                if *running.read() {
                    span { class: "muted", "…" }
                }
            }

            ul { class: "search-results",
                if results.read().is_empty() && !query.read().is_empty() && !*running.read() {
                    li { class: "muted", {tr(lang, "search.no_results")} }
                }
                for m in results.read().clone() {
                    li {
                        class: if m.exact_case { "search-match exact" } else { "search-match" },
                        button {
                            class: "search-match-btn",
                            onclick: {
                                let path = m.relative_path.clone();
                                move |_| {
                                    submit_source_command(
                                        source_sync,
                                        state,
                                        mode_sig,
                                        toasts,
                                        SourceCommand::OpenDocument(path.clone()),
                                    );
                                }
                            },
                            span { class: "match-file", "{m.relative_path.display()}" }
                            span { class: "match-line", ":{m.line_number}" }
                            span { class: "match-text", "{m.line_text}" }
                        }
                    }
                }
            }
        }
    }
}
