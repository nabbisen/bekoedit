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
pub fn relay_js(relay_name: &str) -> String {
    format!(
        r#"
        window.{relay} = (msg) => dioxus.send(msg);
        window.__bk_schema_version = {version};
        (async () => {{
            while (true) {{
                await new Promise(r => setTimeout(r, 86_400_000));
            }}
        }})();
        "#,
        relay = relay_name,
        version = BRIDGE_SCHEMA_VERSION,
    )
}

/// Wraps a relay setup + recv loop to auto-restart on eval failure.
/// `setup_js` must be the output of `relay_js(name)`.
/// The caller provides `handler` as a closure receiving raw JSON values.
///
/// Usage inside a `use_coroutine`:
/// ```
/// use_coroutine(|_: UnboundedReceiver<()>| async move {
///     restart_relay("__bk_relay", |raw| { /* handle */ }).await;
/// });
/// ```
#[allow(dead_code)]
pub const MAX_RELAY_RESTARTS: usize = 10;
