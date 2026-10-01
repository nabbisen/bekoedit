//! One-shot DOM reads for the release-checks driver.
//!
//! Task 039's sighting showed that awaiting a `join` immediately, with
//! nothing else in between, does not protect it: between the page's `return`
//! message waking the spawned task and that task's next poll, a
//! garbage-collection `drop` can free the evaluator (the generational
//! `Owner` a `QueryEntry`'s slab entry holds -- see `focus.rs`'s own
//! `arm_focus_guard_js` doc comment for the full mechanism, confirmed
//! against the real `dioxus-desktop` 0.7.9 sources). Every read here uses the
//! same shape as that fix instead: the page sends its value with
//! `dioxus.send`, then waits for Rust's release, bounded; Rust reads with
//! `recv`, sends the release, and never `join`s.

use std::time::Duration;

use dioxus::prelude::*;
use serde::Deserialize;

const EVAL_TIMEOUT: Duration = Duration::from_secs(3);
/// Longer than `EVAL_TIMEOUT`, so the ordinary path always resolves via the
/// release, not this bound -- it exists only so a dropped `EVAL_TIMEOUT`
/// race (Rust gave up waiting, so it never sends a release) cannot leave the
/// page's promise, and the query it keeps reachable, pending forever (same
/// reasoning as `focus.rs`'s `GUARD_RELEASE_TIMEOUT_MS`).
const RELEASE_TIMEOUT_MS: u64 = EVAL_TIMEOUT.as_millis() as u64 + 1000;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DomSnapshot {
    pub start_screen: bool,
    pub tree_rows: usize,
}

/// What the page sent back: either the body's own return value, or -- if it
/// threw -- the exception's own message, so a thrown script is reported as
/// that error rather than read as a silent timeout.
#[derive(Debug, Deserialize)]
struct EvalEnvelope {
    ok: bool,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    error: String,
}

/// The script that wraps one function `body` (which may itself `return` a
/// value) in the send/wait-for-release envelope every read here shares. The
/// leading `return` matters exactly as it does in `arm_focus_guard_js`: the
/// evaluated function does not resolve, and so cannot be closed and
/// collected, until the release arrives or the bound elapses.
fn eval_envelope_js(body: &str) -> String {
    format!(
        r#"
        return (async () => {{
            let result;
            try {{
                const value = await (async () => {{ {body} }})();
                result = {{ ok: true, value }};
            }} catch (error) {{
                result = {{ ok: false, error: String(error) }};
            }}
            dioxus.send(result);
            // Stay alive until Rust releases this query, bounded so a
            // dropped EVAL_TIMEOUT race (Rust stopped waiting and never
            // sends a release) cannot leave this promise pending forever.
            await Promise.race([
                dioxus.recv(),
                new Promise((resolve) => setTimeout(resolve, {RELEASE_TIMEOUT_MS})),
            ]);
            return null;
        }})();
        "#,
    )
}

/// Evaluates `body` (a function body; `returned` below wraps a bare
/// expression in one `return` statement to reuse this) and decodes its
/// return value as `T`. The one place every read here goes through, so
/// there is one copy of the eval/recv/release shape to re-audit before a
/// Dioxus update (`focus.rs`'s `arm_focus_guard_js` doc comment names the
/// files).
async fn eval_body<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, String> {
    let mut eval = document::eval(&eval_envelope_js(body));
    let envelope: EvalEnvelope = tokio::time::timeout(EVAL_TIMEOUT, eval.recv())
        .await
        .map_err(|_| format!("no answer within {EVAL_TIMEOUT:?}"))?
        .map_err(|error| error.to_string())?;
    // Task 040 §2.1: released after `recv` returns, whether or not the
    // envelope goes on to decode below, and never `join`ed -- there is
    // nothing left to read after this that could lose anything.
    let _ = eval.send(true);
    decode_envelope(envelope)
}

/// What `eval_body` does with the page's envelope once it has it: the body's
/// own return value decoded as `T`, or -- if the body threw -- the exception's
/// message, verbatim, as the `Err`. Pure, so it is tested directly without a
/// live WebView; `eval_body` is the only caller, and the only place a live
/// `document::eval` is involved.
fn decode_envelope<T: serde::de::DeserializeOwned>(envelope: EvalEnvelope) -> Result<T, String> {
    if envelope.ok {
        serde_json::from_value(envelope.value)
            .map_err(|error| format!("could not decode the page's answer: {error}"))
    } else {
        Err(envelope.error)
    }
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
