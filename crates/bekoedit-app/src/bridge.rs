//! WebView bridge utilities (RFC-002).
//!
//! Provides a robust relay-eval bootstrap that:
//! - Sets `window.__bk_relay` to the current eval's `dioxus.send` channel,
//!   bound to the correct context so messages route to the right receiver.
//! - Keeps the eval alive with a long-sleep loop.
//! - Embeds the BRIDGE_SCHEMA_VERSION so the JS side can detect mismatches.

use bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION;
use std::fmt::Display;

const SOURCE_TRACE_ENV: &str = "BEKOEDIT_SOURCE_TRACE";

pub fn trace(event: &str, details: impl Display) {
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
mod tests {
    use super::*;

    #[test]
    fn relay_backoff_caps_without_exhausting() {
        let delays: Vec<_> = (1..=20).map(relay_restart_delay_ms).collect();
        assert_eq!(&delays[..3], &[100, 200, 400]);
        assert!(delays[3..].iter().all(|delay| *delay == 400));
    }

    #[test]
    fn relay_scripts_bind_and_clear_only_the_exact_generation() {
        let install = relay_js("__test_relay", 41);
        let clear = clear_relay_js("__test_relay", 41);
        assert!(install.contains("relay.__bkGeneration = 41"));
        assert!(install.contains("relayGenerationReady"));
        assert!(clear.contains("relay.__bkGeneration === 41"));
        assert!(clear.contains("delete window.__test_relay"));
    }

    #[test]
    fn a_literal_escapes_both_line_separators_and_round_trips() {
        let text = "a\u{2028}b\u{2029}c \"q\" \\ </script> \u{1} é日本🙂\r\n";
        let literal = js_string_literal(text);
        assert!(literal.starts_with('"') && literal.ends_with('"'));
        assert!(literal.contains("\\u2028") && literal.contains("\\u2029"));
        assert!(!literal.contains('\u{2028}') && !literal.contains('\u{2029}'));
        // A JSON parser (which follows the same escape rules as a JS string
        // literal for these) recovers the original exactly.
        assert_eq!(serde_json::from_str::<String>(&literal).unwrap(), text);
    }
}
