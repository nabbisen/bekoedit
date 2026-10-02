use super::*;

fn envelope(ok: bool, value: serde_json::Value, error: &str) -> EvalEnvelope {
    EvalEnvelope {
        ok,
        value,
        error: error.to_string(),
    }
}

#[test]
fn decode_envelope_returns_the_bodys_value_when_it_succeeds() {
    let result: Result<u32, String> = decode_envelope(envelope(true, serde_json::json!(42), ""));
    assert_eq!(result, Ok(42));
}

/// Task 040 §2.1's binding requirement: a thrown error is reported as that
/// error, verbatim, not folded into the generic timeout path.
/// `decode_envelope` is the one place that decision is made, pure, so it is
/// tested without a live WebView.
#[test]
fn decode_envelope_reports_a_thrown_error_verbatim() {
    let result: Result<u32, String> =
        decode_envelope(envelope(false, serde_json::Value::Null, "boom: it threw"));
    assert_eq!(result, Err("boom: it threw".to_string()));
}

#[test]
fn decode_envelope_reports_an_undecodable_value_without_pretending_it_succeeded() {
    let result: Result<bool, String> =
        decode_envelope(envelope(true, serde_json::json!("not a bool"), ""));
    assert!(result.is_err(), "{result:?}");
}

/// Task 040 §2.1: the evaluated function must return its promise, and that
/// promise must not settle until Rust's release arrives or its own bound
/// elapses -- the same shape `source_sync::focus`'s `arm_focus_guard_js`
/// test proves, string-level, since nothing here can run without a live
/// WebView.
#[test]
fn the_envelope_script_returns_its_promise_and_waits_for_release_with_a_bound() {
    let script = eval_envelope_js("return 1;");
    assert!(
        script.contains("return (async () => {"),
        "the evaluated function must return its promise: {script}"
    );
    assert!(script.contains("dioxus.send(result)"), "{script}");
    assert!(script.contains("dioxus.recv()"), "{script}");
    assert!(
        script.contains(&format!("setTimeout(resolve, {RELEASE_TIMEOUT_MS})")),
        "{script}"
    );
    assert!(
        RELEASE_TIMEOUT_MS > EVAL_TIMEOUT.as_millis() as u64,
        "the page's own bound must outlast EVAL_TIMEOUT, or the ordinary path \
         could resolve via the bound instead of the release"
    );
}

/// A thrown body is caught and sent as a failed envelope, not left to reject
/// the outer promise (which `dioxus.send` would never see).
#[test]
fn the_envelope_script_catches_a_thrown_body_into_a_failed_envelope() {
    let script = eval_envelope_js("throw new Error('boom');");
    assert!(script.contains("throw new Error('boom');"), "{script}");
    assert!(script.contains("catch (error)"), "{script}");
    assert!(
        script.contains("ok: false, error: String(error)"),
        "{script}"
    );
}

/// The body is inlined into its own inner IIFE, so its `return` statement is
/// the inner function's, not the outer envelope's -- it may still `return` a
/// value (task 040 §2.1's last point).
#[test]
fn the_envelope_script_lets_the_body_return_its_own_value() {
    let script = eval_envelope_js("return 42;");
    assert!(
        script.contains("(async () => { return 42; })()"),
        "{script}"
    );
}

/// Task 040 §2.1's second point: Rust sends the release only after its
/// `recv` returns. `eval_body` itself needs a live WebView to run, so this
/// is a source-order check, not a call-order one -- the same convention
/// task 039's own release-ordering test uses. Reads `bridge.rs` from this
/// separate file, not from inside it: a self-referential `include_str!`
/// would also match its own assertion strings below.
#[test]
fn the_release_is_sent_only_after_the_envelope_is_received() {
    let source = include_str!("../bridge.rs");
    let recv_at = source
        .find("eval.recv()")
        .expect("the envelope is read with eval.recv()");
    let send_at = source
        .find("let _ = eval.send(true);")
        .expect("the release is sent with eval.send(true)");
    assert!(
        recv_at < send_at,
        "the release (byte {send_at}) must be sent after the envelope is \
         received (byte {recv_at}), not before"
    );
}

/// Task 040 §2.1's first point: `eval_body` must never `join` an eval -- the
/// task 039 sighting showed that an immediately-awaited `join`, with nothing
/// else in between, is not protected from the eval-lifetime race either.
#[test]
fn eval_body_never_joins() {
    let source = include_str!("../bridge.rs");
    assert!(
        !source.contains(".join("),
        "bridge.rs must never join an eval (task 040 section 2.1): {source}"
    );
}

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
