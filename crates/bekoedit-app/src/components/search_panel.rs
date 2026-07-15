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
use crate::state::SearchOpen;

fn run_search(
    root: Option<std::path::PathBuf>,
    query: String,
    mut results: Signal<Vec<SearchMatch>>,
    mut running: Signal<bool>,
    mut generation: Signal<u64>,
    mut searched: Signal<bool>,
) {
    let Some(root) = root else { return };
    if query.trim().is_empty() {
        results.set(Vec::new());
        searched.set(false);
        return;
    }
    let search_generation = {
        let mut current = generation.write();
        *current = current.saturating_add(1);
        *current
    };
    searched.set(false);
    running.set(true);
    spawn(async move {
        let matches = search_workspace(&root, &query, 200);
        if *generation.read() == search_generation {
            results.set(matches);
            running.set(false);
            searched.set(true);
        }
    });
}

#[component]
pub fn SearchPanel() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mode_sig = use_context::<Signal<EditorMode>>();
    let source_sync = use_context::<Signal<SourceSyncState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mut query = use_signal(String::new);
    let mut results: Signal<Vec<SearchMatch>> = use_signal(Vec::new);
    let mut running = use_signal(|| false);
    let mut generation = use_signal(|| 0_u64);
    let mut searched = use_signal(|| false);
    let mut search_open = use_context::<SearchOpen>().0;

    let root = state.read().workspace.as_ref().map(|w| w.root_path.clone());
    let root_for_key = root.clone();
    let root_for_click = root.clone();
    let search_available = root.is_some();

    use_effect(|| {
        document::eval(
            r#"requestAnimationFrame(() => document.getElementById('workspace-search-input')?.focus())"#,
        );
    });

    rsx! {
        section {
            class: "search-panel",
            role: "search",
            aria_label: tr(lang, "search.label"),

            div { class: "search-panel-header",
                h2 { class: "outline-title", {tr(lang, "search.label")} }
                button {
                    class: "search-close",
                    aria_label: tr(lang, "search.close"),
                    title: tr(lang, "search.close"),
                    onclick: move |_| search_open.set(false),
                    "×"
                }
            }

            div { class: "search-row",
                input {
                    id: "workspace-search-input",
                    r#type: "search",
                    class: "search-input",
                    autofocus: true,
                    placeholder: tr(lang, "search.placeholder"),
                    aria_label: tr(lang, "search.placeholder"),
                    value: "{query}",
                    oninput: move |evt| {
                        query.set(evt.value());
                        results.set(Vec::new());
                        running.set(false);
                        searched.set(false);
                        let next = generation.read().saturating_add(1);
                        generation.set(next);
                    },
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            run_search(
                                root_for_key.clone(),
                                query.read().clone(),
                                results,
                                running,
                                generation,
                                searched,
                            );
                        }
                    },
                }
                button {
                    class: "btn-secondary",
                    disabled: !search_available || query.read().trim().is_empty(),
                    onclick: move |_| {
                        run_search(
                            root_for_click.clone(),
                            query.read().clone(),
                            results,
                            running,
                            generation,
                            searched,
                        );
                    },
                    {tr(lang, "search.submit")}
                }
                if *running.read() {
                    span { class: "muted", "…" }
                }
            }

            ul { class: "search-results",
                if *searched.read() && results.read().is_empty() && !*running.read() {
                    li { class: "muted", {tr(lang, "search.empty")} }
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
