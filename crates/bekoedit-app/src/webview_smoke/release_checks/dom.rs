//! One-shot DOM reads for the release-checks driver: a single `document::eval`
//! that returns a value, joined -- never a bare send then drop (the Dioxus
//! 0.7.9 hazard `transport.rs` documents).

use std::time::Duration;

use dioxus::prelude::*;
use serde::Deserialize;

const EVAL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DomSnapshot {
    pub start_screen: bool,
    pub tree_rows: usize,
}

async fn returned<T: serde::de::DeserializeOwned>(expression: &str) -> Result<T, String> {
    let script = format!("return (async () => ({expression}))();");
    tokio::time::timeout(EVAL_TIMEOUT, document::eval(&script).join::<T>())
        .await
        .map_err(|_| format!("the page did not answer {expression} within {EVAL_TIMEOUT:?}"))?
        .map_err(|error| format!("reading {expression} failed: {error}"))
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

/// Runs `script` as a function body (it may `return` a value), joined like the
/// other reads here. For a whole script rather than one expression.
pub(super) async fn run_script<T: serde::de::DeserializeOwned>(script: &str) -> Result<T, String> {
    tokio::time::timeout(EVAL_TIMEOUT, document::eval(script).join::<T>())
        .await
        .map_err(|_| format!("the page did not finish the script within {EVAL_TIMEOUT:?}"))?
        .map_err(|error| format!("running the script failed: {error}"))
}

/// One expression's JSON value, for the paste probe's calls (`paste_probe.js`).
pub(super) async fn value_of(expression: &str) -> Result<serde_json::Value, String> {
    returned(expression).await
}
