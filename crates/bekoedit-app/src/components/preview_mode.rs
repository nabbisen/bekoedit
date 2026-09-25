//! Preview Mode (RFC-012): read-only rendered Markdown.
//!
//! The HTML comes from `render_preview_html`, which shows raw HTML from
//! the document as escaped text (requirements §17.2), except a bare inline
//! `<br>` (a line break), and drops link and image destinations that are not
//! on its allowlist, such as `javascript:` (invariant 10, task 030). So
//! injecting it into the DOM here cannot execute document-controlled
//! scripts, and the exception and the allowlist are that renderer's, not
//! this component's.

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
