//! Preview Mode (RFC-012): read-only rendered Markdown.
//!
//! The HTML comes from `render_preview_html`, which escapes all raw HTML
//! from the document (requirements §17.2), so injecting it into the DOM
//! here cannot execute document-controlled scripts.

use dioxus::prelude::*;

use bekoedit_core::AppState;

#[component]
pub fn PreviewMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let html = state
        .read()
        .session
        .as_ref()
        .map(|s| s.preview_html())
        .unwrap_or_default();

    rsx! {
        article {
            class: "preview",
            "data-source-focus-launch-region": "preview",
            dangerous_inner_html: "{html}",
        }
    }
}
