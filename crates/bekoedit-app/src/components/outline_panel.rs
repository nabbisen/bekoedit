//! Outline panel (RFC-010): heading navigation derived from the
//! `MarkdownIndex`. Clicking a heading in Text or Split mode scrolls
//! CM6 to that heading's line; in Form and Preview modes it scrolls the
//! rendered surface.

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};

#[component]
pub fn OutlinePanel() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let headings = state
        .read()
        .session
        .as_ref()
        .map(|s| s.index.headings.clone())
        .unwrap_or_default();

    rsx! {
        aside {
            class: "outline-panel",
            role: "navigation",
            aria_label: tr(lang, "outline.label"),
            h2 { class: "outline-title", {tr(lang, "outline.title")} }
            if headings.is_empty() {
                p { class: "muted", {tr(lang, "outline.empty")} }
            } else {
                ul { class: "outline-list",
                    for heading in headings {
                        li {
                            key: "{heading.source_range.start}",
                            class: "outline-h{heading.level}",
                            button {
                                class: "outline-btn",
                                onclick: {
                                    let offset = heading.source_range.start;
                                    move |_| {
                                        // Scroll CM6 (Text/Split) or the window (Preview/Form)
                                        // to the heading's byte offset.
                                        let js = format!(r#"
                                            if (window.__bk && window.__bk._view) {{
                                                const v = window.__bk._view;
                                                const pos = Math.min({offset}, v.state.doc.length);
                                                v.dispatch({{ selection: {{ anchor: pos }},
                                                              scrollIntoView: true }});
                                                v.focus();
                                            }} else {{
                                                const el = document.querySelector('.preview h{level}');
                                                if (el) el.scrollIntoView({{ behavior:'smooth' }});
                                            }}
                                        "#, offset = offset, level = heading.level);
                                        document::eval(&js);
                                    }
                                },
                                style: "padding-left: {(heading.level as u32 - 1) * 12}px",
                                {heading.text.clone()}
                            }
                        }
                    }
                }
            }
        }
    }
}
