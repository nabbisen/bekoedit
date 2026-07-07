//! Backlinks panel (RFC-034): shows which workspace files link to the
//! current document. Results are computed on demand (Enter in the panel
//! or when the panel opens) via a background `spawn` task.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_fs::{BacklinkEntry, find_backlinks};

use crate::i18n::{Lang, tr};

#[component]
pub fn BacklinksPanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let links: Signal<Vec<BacklinkEntry>> = use_signal(Vec::new);
    let scanning = use_signal(|| false);

    // Compute backlinks when the component mounts or the document changes.
    let doc_path = state.read().session.as_ref().map(|s| s.path.clone());
    let ws_root = state.read().workspace.as_ref().map(|w| w.root_path.clone());
    use_effect(move || {
        if let (Some(ref root), Some(ref doc)) = (ws_root.clone(), doc_path.clone()) {
            let r = root.clone();
            let d = doc.clone();
            let mut lk = links;
            let mut sc = scanning;
            sc.set(true);
            spawn(async move {
                let rel = d.strip_prefix(&r).unwrap_or(&d).to_path_buf();
                let results = find_backlinks(&r, &rel);
                lk.set(results);
                sc.set(false);
            });
        }
    });

    rsx! {
        aside {
            class: "outline-panel",
            role: "complementary",
            aria_label: tr(lang, "backlinks.label"),
            h2 { class: "outline-title", {tr(lang, "backlinks.title")} }
            if *scanning.read() {
                p { class: "muted", "…" }
            } else if links.read().is_empty() {
                p { class: "muted", {tr(lang, "backlinks.empty")} }
            } else {
                p { class: "muted",
                    "{links.read().len()} {tr(lang, \"backlinks.count_suffix\")}"
                }
                ul { class: "outline-list",
                    for entry in links.read().clone() {
                        li {
                            key: "{entry.source_path.display()}-{entry.line_number}",
                            button {
                                class: "outline-btn",
                                onclick: {
                                    let path = entry.source_path.clone();
                                    move |_| {
                                        let _ = state.write().open_document(&path);
                                    }
                                },
                                span { class: "match-file",
                                    "{entry.source_path.display()}"
                                }
                                span { class: "match-line muted",
                                    ":{entry.line_number}"
                                }
                                span { class: "match-text",
                                    "{entry.context}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
