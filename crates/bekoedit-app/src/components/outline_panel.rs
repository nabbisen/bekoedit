//! Outline panel: heading navigation + section move operations (RFC-010/029).
//!
//! Each heading row shows its text, an up-arrow button, and a down-arrow
//! button. Clicking a heading scrolls the editor to that position; the
//! arrows reorder sibling sections without touching unrelated content.

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn OutlinePanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let headings = state
        .read()
        .session
        .as_ref()
        .map(|s| s.index.headings.clone())
        .unwrap_or_default();
    let n = headings.len();

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
                    for (idx, heading) in headings.iter().enumerate() {
                        li {
                            key: "{heading.source_range.start}",
                            class: "outline-h{heading.level}",
                            // Navigate to heading (scroll CM6 / preview)
                            button {
                                class: "outline-btn",
                                style: "padding-left: {(heading.level as u32 - 1) * 12}px",
                                onclick: {
                                    let offset = heading.source_range.start;
                                    let level  = heading.level;
                                    move |_| {
                                        let js = format!(r#"
                                            if (window.__bk?._view) {{
                                                const v = window.__bk._view;
                                                const pos = Math.min({offset}, v.state.doc.length);
                                                v.dispatch({{ selection: {{ anchor: pos }},
                                                              scrollIntoView: true }});
                                                v.focus();
                                            }} else {{
                                                const els = document.querySelectorAll('.preview h{level}');
                                                if (els.length) els[0].scrollIntoView({{ behavior:'smooth' }});
                                            }}
                                        "#, offset = offset, level = level);
                                        document::eval(&js);
                                    }
                                },
                                {heading.text.clone()}
                            }
                            // Move section up (RFC-029)
                            if idx > 0 {
                                button {
                                    class: "section-move-btn",
                                    title: tr(lang, "outline.move_up"),
                                    aria_label: tr(lang, "outline.move_up"),
                                    onclick: move |_| {
                                        let _ = state.write().move_section_up(idx, now_ms());
                                    },
                                    "↑"
                                }
                            }
                            // Move section down (RFC-029)
                            if idx + 1 < n {
                                button {
                                    class: "section-move-btn",
                                    title: tr(lang, "outline.move_down"),
                                    aria_label: tr(lang, "outline.move_down"),
                                    onclick: move |_| {
                                        let _ = state.write().move_section_down(idx, now_ms());
                                    },
                                    "↓"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
