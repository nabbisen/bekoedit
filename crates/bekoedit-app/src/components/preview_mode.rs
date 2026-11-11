//! Preview Mode (RFC-012): read-only rendered Markdown.
//!
//! The HTML comes from `render_preview_html`, which shows raw HTML from
//! the document as escaped text (requirements §17.2), except a bare inline
//! `<br>` (a line break), and drops link and image destinations that are not
//! on its allowlist, such as `javascript:` (invariant 10, task 030). So
//! injecting it into the DOM here cannot execute document-controlled
//! scripts, and the exception and the allowlist are that renderer's, not
//! this component's.
//!
//! Clicks on links in this HTML never reach an OS opener directly:
//! `link_guard` cancels them and `decide_link_click` decides (invariant 11).

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::i18n::{Lang, tr};

/// Gives each task-list checkbox in rendered Preview HTML an accessible name
/// (RFC-050 slice 1). `pulldown-cmark` writes them as a bare
/// `<input disabled="" type="checkbox"/>` (with ` checked=""` when ticked), which
/// a screen reader can only announce as an unnamed checkbox. The renderer is the
/// Markdown crate's and knows no language, so the label is added here, from
/// `i18n.rs`. The tag text it looks for is the renderer's own; a document's raw
/// HTML never matches it, because the renderer escapes that.
pub(crate) fn name_task_checkboxes(html: &str, label: &str) -> String {
    let label = label.replace('&', "&amp;").replace('"', "&quot;");
    html.replace(
        "<input disabled=\"\" type=\"checkbox\"",
        &format!("<input aria-label=\"{label}\" disabled=\"\" type=\"checkbox\""),
    )
}

#[component]
pub fn PreviewMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let html = state
        .read()
        .session
        .as_ref()
        .map(|s| name_task_checkboxes(&s.preview_html(), tr(lang, "block.task_item")))
        .unwrap_or_default();

    rsx! {
        article {
            class: "preview",
            "data-source-focus-launch-region": "preview",
            dangerous_inner_html: "{html}",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_checkbox_gets_the_label_whether_ticked_or_not() {
        let html = bekoedit_markdown::render_preview_html("- [ ] open\n- [x] done\n- plain\n");
        assert_eq!(html.matches("<input").count(), 2, "{html}");
        let named = name_task_checkboxes(&html, "Task done");
        assert_eq!(
            named
                .matches("<input aria-label=\"Task done\" disabled=\"\" type=\"checkbox\"")
                .count(),
            2,
            "{named}"
        );
        assert!(
            named.contains("checked=\"\""),
            "the ticked state is kept: {named}"
        );
        assert_eq!(
            named.replace(" aria-label=\"Task done\"", ""),
            html,
            "nothing else changes"
        );
    }

    #[test]
    fn a_document_cannot_forge_a_checkbox_to_be_relabelled() {
        // Raw HTML is escaped by the renderer, so it never matches the tag text.
        let html =
            bekoedit_markdown::render_preview_html("<input disabled=\"\" type=\"checkbox\"/>\n");
        assert!(html.contains("&lt;input"), "{html}");
        assert_eq!(name_task_checkboxes(&html, "Task done"), html);
    }

    #[test]
    fn a_label_with_quotes_cannot_break_out_of_the_attribute() {
        let html = bekoedit_markdown::render_preview_html("- [ ] x\n");
        let named = name_task_checkboxes(&html, "a\" onfocus=\"x");
        assert!(
            named.contains("aria-label=\"a&quot; onfocus=&quot;x\""),
            "{named}"
        );
    }
}
