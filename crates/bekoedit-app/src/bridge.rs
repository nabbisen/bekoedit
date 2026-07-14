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

/// JavaScript that installs a named relay function and keeps the eval
/// context alive. `relay_name` is the `window` property to set
/// (e.g. `"__bk_relay"` or `"__bk_shortcut_relay"`).
pub fn relay_js(relay_name: &str, generation: u64) -> String {
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
        relay = relay_name,
        generation = generation,
        version = BRIDGE_SCHEMA_VERSION,
    )
}

/// Clears only the retired relay generation, never a newer replacement.
pub fn clear_relay_js(relay_name: &str, generation: u64) -> String {
    format!(
        r#"
        (() => {{
            const relay = window.{relay};
            if (relay && relay.__bkGeneration === {generation}) {{
                delete window.{relay};
            }}
        }})();
        "#,
        relay = relay_name,
        generation = generation,
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
}
