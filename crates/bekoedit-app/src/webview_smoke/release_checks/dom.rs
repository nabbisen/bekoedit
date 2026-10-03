//! One-shot DOM reads for the release-checks driver.
//!
//! Every read here goes through `crate::bridge::eval_body`, the bounded,
//! release-after-recv, never-`join`ed one-shot eval shared across the app
//! (task 039/040's eval-lifetime rules; task 047 Part B generalised it from
//! this module into `bridge.rs` so the inline-formatting toolbar's one-shot
//! selection read could use the exact same shape).

use serde::Deserialize;

use crate::bridge::eval_body;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DomSnapshot {
    pub start_screen: bool,
    pub tree_rows: usize,
}

/// Names `expression` in a failed read's error, as `join`'s own error did
/// before task 040 -- pure, so the naming survives independent of a live
/// WebView.
fn describe_read_failure(expression: &str, error: String) -> String {
    format!("reading {expression} failed: {error}")
}

/// Names the failure as the script's, not one specific expression's -- the
/// `run_script` counterpart to [`describe_read_failure`].
fn describe_script_failure(error: String) -> String {
    format!("running the script failed: {error}")
}

async fn returned<T: serde::de::DeserializeOwned>(expression: &str) -> Result<T, String> {
    eval_body(&format!("return ({expression});"))
        .await
        .map_err(|error| describe_read_failure(expression, error))
}

/// What is on screen: whether the Start Screen is rendered, and how many
/// workspace-tree rows there are.
pub(super) async fn snapshot() -> Result<DomSnapshot, String> {
    returned(
        "({ startScreen: document.querySelector('.start-screen') !== null, \
         treeRows: document.querySelectorAll('[data-tree-row]').length })",
    )
    .await
}

/// The source editor is mounted, focused, and not showing a status marker
/// (the same condition task 023's driver waits on).
pub(super) async fn editor_focused() -> Result<bool, String> {
    returned(
        "Boolean(window.__bk?._view && window.__bk._view.dom?.isConnected && \
         window.__bk._view.hasFocus && \
         !document.querySelector('[data-source-focus-launch-region=\"text\"] .source-editor-status'))",
    )
    .await
}

/// Whether the editor's own text contains `needle` -- used only to wait for a
/// typed edit to have arrived, never for the byte comparison.
pub(super) async fn editor_contains(needle: &str) -> Result<bool, String> {
    let needle = crate::bridge::js_string_literal(needle);
    returned(&format!(
        "Boolean(window.__bk?._view?.state.doc.toString().includes({needle}))"
    ))
    .await
}

/// Form Mode is showing its one seeded paragraph field (`toolbar_probe`).
pub(super) async fn form_paragraph_present() -> Result<bool, String> {
    returned("document.querySelector('.form-mode .paragraph-input') !== null").await
}

/// Whether the seeded paragraph field's own live value is exactly
/// `expected` -- used to wait for real keystrokes (task 049 §2.1) to have
/// landed before the scenario's own act under test.
pub(super) async fn paragraph_field_value_is(expected: &str) -> Result<bool, String> {
    let expected = crate::bridge::js_string_literal(expected);
    returned(&format!(
        "(() => {{ const el = document.querySelector('.form-mode .paragraph-input'); \
         return !!el && el.value === {expected}; }})()"
    ))
    .await
}

/// `document.activeElement`'s own tag, id, first two classes, and
/// whether it is inside `.form-mode` -- never its value or text. Used
/// only to name what has focus (`save_pending_field`'s own focus-thief
/// report), never to read or compare field content.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActiveElementDescription {
    tag: String,
    id: String,
    classes: Vec<String>,
    in_form_mode: bool,
}

impl ActiveElementDescription {
    pub(super) fn trace_line(&self) -> String {
        format!(
            "tag={:?} id={:?} classes={:?} in_form_mode={}",
            self.tag, self.id, self.classes, self.in_form_mode
        )
    }

    /// Whether this describes the Form Mode paragraph field itself --
    /// never its exact `id`, which changes with the field's own
    /// content (task 052 §2.2's own reason for asking "is it the
    /// field", not "is it this specific id").
    pub(super) fn is_paragraph_field(&self) -> bool {
        self.tag == "textarea"
            && self.in_form_mode
            && self.classes.iter().any(|class| class == "paragraph-input")
    }
}

/// Builds [`ActiveElementDescription`] from `document.activeElement` --
/// never `el.value` or any text, only what identifies it.
const ACTIVE_ELEMENT_JS: &str = "(() => { const el = document.activeElement; \
     const classes = el && el.className \
         ? String(el.className).split(/\\s+/).filter(Boolean).slice(0, 2) : []; \
     return { tag: el ? el.tagName.toLowerCase() : '', id: (el && el.id) || '', \
         classes, inFormMode: !!(el && el.closest && el.closest('.form-mode')) }; })()";

/// What has focus right now: `document.activeElement`'s tag, id, first
/// two classes, and whether it is inside `.form-mode`. Never its value
/// or text.
pub(super) async fn active_element() -> Result<ActiveElementDescription, String> {
    returned(ACTIVE_ELEMENT_JS).await
}

/// The Preview tab is the selected mode tab.
pub(super) async fn preview_selected() -> Result<bool, String> {
    returned(
        "document.querySelector('[data-source-focus-launch=\"mode-preview\"].active[aria-selected=\"true\"]') !== null",
    )
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PreviewLink {
    pub text: String,
    pub href: String,
}

/// Every anchor in the rendered Preview, with its text and its `href`
/// attribute exactly as the page holds it.
pub(super) async fn preview_links() -> Result<Vec<PreviewLink>, String> {
    returned(
        "Array.from(document.querySelectorAll('article.preview a')).map((a) => \
         ({ text: a.textContent, href: a.getAttribute('href') ?? '' }))",
    )
    .await
}

/// Adds `<p><a href=…>text</a></p>` to the Preview article, so a click can be
/// sent to a link the renderer would never produce (task 033: a network-path
/// `href`, which task 030 drops at render time). `false` if there is no
/// Preview article.
pub(super) async fn inject_preview_anchor(text: &str, href: &str) -> Result<bool, String> {
    let text = crate::bridge::js_string_literal(text);
    let href = crate::bridge::js_string_literal(href);
    returned(&format!(
        "(() => {{ const root = document.querySelector('article.preview'); \
         if (!root) return false; \
         const p = document.createElement('p'); \
         const a = document.createElement('a'); \
         a.setAttribute('href', {href}); a.textContent = {text}; \
         p.appendChild(a); root.appendChild(p); return true; }})()"
    ))
    .await
}

/// Removes the link guard's own listeners from the page (task 035), the same
/// two calls task 032's JS test makes. `false` if there was no guard to remove.
pub(super) async fn remove_link_guard() -> Result<bool, String> {
    returned(
        "(() => { const guard = window.__bk_link_guard; \
         if (typeof guard !== 'function') return false; \
         window.removeEventListener('click', guard, true); \
         window.removeEventListener('auxclick', guard, true); \
         return true; })()",
    )
    .await
}

/// Runs `script` as a function body (it may `return` a value). For a whole
/// script rather than one expression.
pub(super) async fn run_script<T: serde::de::DeserializeOwned>(script: &str) -> Result<T, String> {
    eval_body(script).await.map_err(describe_script_failure)
}

/// One expression's JSON value, for the paste probe's calls (`paste_probe.js`).
pub(super) async fn value_of(expression: &str) -> Result<serde_json::Value, String> {
    returned(expression).await
}

#[cfg(test)]
mod tests;
