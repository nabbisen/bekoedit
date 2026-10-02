//! WebView bridge utilities (RFC-002).
//!
//! Provides a robust relay-eval bootstrap that:
//! - Sets `window.__bk_relay` to the current eval's `dioxus.send` channel,
//!   bound to the correct context so messages route to the right receiver.
//! - Keeps the eval alive with a long-sleep loop.
//! - Embeds the BRIDGE_SCHEMA_VERSION so the JS side can detect mismatches.

use bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION;
use dioxus::prelude::*;
use std::fmt::Display;
use std::time::Duration;

const SOURCE_TRACE_ENV: &str = "BEKOEDIT_SOURCE_TRACE";

pub fn trace(event: &str, details: impl Display) {
    // Task 036: in a trusted-click run, `source.focus.*` events also go to that
    // run's log (a no-op, and no formatting, in every other run), so the whole
    // focus path is visible, not only the half the page reports.
    crate::webview_smoke::record_source_trace(event, &details);
    if std::env::var_os(SOURCE_TRACE_ENV).is_some() {
        eprintln!("[bekoedit-source-trace] {event} {details}");
    }
}

/// A JavaScript **string literal** for `text`, safe to paste into evaluated
/// source (task 031). The one helper every Rust-to-page interpolation of JSON
/// goes through: serialize the payload, pass *that string* here, and have the
/// page `JSON.parse` it -- so the payload reaches the page as data, not as
/// program text the JavaScript parser must tokenize.
///
/// `serde_json` escapes `"`, `\` and the C0 controls, but leaves U+2028 and
/// U+2029 literal. ES2019 made both legal inside string literals, so that is
/// correct on today's engines only by an engine-version detail; they are
/// emitted as `\u2028` and `\u2029` so correctness does not depend on it.
pub fn js_string_literal(text: &str) -> String {
    serde_json::to_string(text)
        .expect("a string serializes")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// JavaScript that installs a named relay function and keeps the eval
/// context alive. `relay_name` is the `window` property to set
/// (e.g. `"__bk_relay"` or `"__bk_shortcut_relay"`).
pub fn relay_js(relay_name: &str, generation: u64) -> String {
    let relay = relay_name;
    let version = BRIDGE_SCHEMA_VERSION;
    format!(
        r#"
        const relay = (msg) => dioxus.send(msg);
        relay.__bkGeneration = {generation};
        window.{relay} = relay;
        window.__bk_schema_version = {version};
        dioxus.send(JSON.stringify({{
            type: "relayGenerationReady",
            generation: {generation}
        }}));
        (async () => {{
            while (true) {{
                await new Promise(r => setTimeout(r, 86_400_000));
            }}
        }})();
        "#,
    )
}

/// Clears only the retired relay generation, never a newer replacement.
pub fn clear_relay_js(relay_name: &str, generation: u64) -> String {
    let relay = relay_name;
    format!(
        r#"
        (() => {{
            const relay = window.{relay};
            if (relay && relay.__bkGeneration === {generation}) {{
                delete window.{relay};
            }}
        }})();
        "#,
    )
}

const EVAL_TIMEOUT: Duration = Duration::from_secs(3);
/// Longer than `EVAL_TIMEOUT`, so the ordinary path always resolves via the
/// release, not this bound -- it exists only so a dropped `EVAL_TIMEOUT`
/// race (Rust gave up waiting, so it never sends a release) cannot leave the
/// page's promise, and the query it keeps reachable, pending forever (same
/// reasoning as `source_sync::focus`'s `GUARD_RELEASE_TIMEOUT_MS`).
const RELEASE_TIMEOUT_MS: u64 = EVAL_TIMEOUT.as_millis() as u64 + 1000;

/// What the page sent back: either the body's own return value, or -- if it
/// threw -- the exception's own message, so a thrown script is reported as
/// that error rather than read as a silent timeout.
#[derive(Debug, serde::Deserialize)]
struct EvalEnvelope {
    ok: bool,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    error: String,
}

/// The script that wraps one function `body` (which may itself `return` a
/// value) in a send/wait-for-release envelope (tasks 039/040's eval-lifetime
/// rules): the evaluated function does not resolve, and so cannot be closed
/// and collected, until the release arrives or the bound elapses, which is
/// what keeps a one-shot read safe from the Dioxus 0.7.9 drop race
/// `source_sync::focus`'s `arm_focus_guard_js` doc comment documents in full.
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

/// What `eval_body` does with the page's envelope once it has it: the body's
/// own return value decoded as `T`, or -- if the body threw -- the
/// exception's message, verbatim, as the `Err`. Pure, so it is tested
/// directly without a live WebView.
fn decode_envelope<T: serde::de::DeserializeOwned>(envelope: EvalEnvelope) -> Result<T, String> {
    if envelope.ok {
        serde_json::from_value(envelope.value)
            .map_err(|error| format!("could not decode the page's answer: {error}"))
    } else {
        Err(envelope.error)
    }
}

/// Evaluates `body` (a function body; it may `return` a value) and decodes
/// its return value as `T`. One self-contained, bounded, release-after-recv,
/// never-`join`ed round trip per call (tasks 039/040) -- the shared place
/// every one-shot read or command in this app goes through, so there is one
/// copy of the eval/recv/release shape to re-audit before a Dioxus update.
/// Never leaves a lingering `window`-bound relay or keep-alive loop behind:
/// unlike a persistent relay, nothing here outlives the single call that
/// created it (task 047 Part B, moved here from `release_checks::dom` since
/// the inline-formatting toolbar needed the exact same one-shot shape).
pub async fn eval_body<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, String> {
    let mut eval = document::eval(&eval_envelope_js(body));
    let envelope: EvalEnvelope = tokio::time::timeout(EVAL_TIMEOUT, eval.recv())
        .await
        .map_err(|_| format!("no answer within {EVAL_TIMEOUT:?}"))?
        .map_err(|error| error.to_string())?;
    // Released after `recv` returns, whether or not the envelope goes on to
    // decode below, and never `join`ed -- there is nothing left to read
    // after this that could lose anything.
    let _ = eval.send(true);
    decode_envelope(envelope)
}

pub const RELAY_RESTART_BASE_MS: u64 = 100;
pub const RELAY_RESTART_CAP_MS: u64 = 400;

/// Returns a capped retry delay without ever exhausting the relay owner.
pub fn relay_restart_delay_ms(consecutive_failures: u32) -> u64 {
    let shift = consecutive_failures.saturating_sub(1).min(2);
    RELAY_RESTART_BASE_MS
        .saturating_mul(1_u64 << shift)
        .min(RELAY_RESTART_CAP_MS)
}

#[cfg(test)]
mod tests;
