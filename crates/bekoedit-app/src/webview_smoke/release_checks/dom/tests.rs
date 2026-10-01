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
/// error, verbatim, not folded into the generic timeout path. `decode_envelope`
/// is the one place that decision is made, pure, so it is tested without a
/// live WebView.
#[test]
fn decode_envelope_reports_a_thrown_error_verbatim() {
    let result: Result<u32, String> =
        decode_envelope(envelope(false, serde_json::Value::Null, "boom: it threw"));
    assert_eq!(result, Err("boom: it threw".to_string()));
}

#[test]
fn decode_envelope_reports_an_undecodable_value_without_pretending_it_succeeded() {
    // ok: true, but the value cannot decode as the type the caller asked for.
    let result: Result<bool, String> =
        decode_envelope(envelope(true, serde_json::json!("not a bool"), ""));
    assert!(result.is_err(), "{result:?}");
}

/// The call site (`returned`) must still name the expression that failed, as
/// `join`'s own error did before task 040 -- otherwise a release-checks
/// failure would stop saying which read failed.
#[test]
fn describe_read_failure_names_the_expression() {
    let message = describe_read_failure("window.__bk?._view", "boom".to_string());
    assert_eq!(message, "reading window.__bk?._view failed: boom");
}

#[test]
fn describe_script_failure_does_not_claim_to_be_one_expression() {
    let message = describe_script_failure("boom".to_string());
    assert_eq!(message, "running the script failed: boom");
}

/// Task 040 §2.1: the evaluated function must return its promise, and that
/// promise must not settle until Rust's release arrives or its own bound
/// elapses -- the same shape `focus.rs`'s `arm_focus_guard_js` test proves,
/// string-level, since nothing here can run without a live WebView.
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

/// `run_script` must still accept a whole function body that may `return` a
/// value (task 040 §2.1's last point): the body is inlined into its own inner
/// IIFE, so its `return` statement is the inner function's, not the outer
/// envelope's.
#[test]
fn the_envelope_script_lets_the_body_return_its_own_value() {
    let script = eval_envelope_js("return 42;");
    assert!(
        script.contains("(async () => { return 42; })()"),
        "{script}"
    );
}

/// Task 040 §2.1's second point: Rust sends the release only after its `recv`
/// returns. `eval_body` itself needs a live WebView to run, so this is a
/// source-order check, not a call-order one -- the same convention task 039's
/// `the_release_is_sent_only_after_the_acknowledgement_is_received` uses.
#[test]
fn the_release_is_sent_only_after_the_envelope_is_received() {
    let source = include_str!("../dom.rs");
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

/// Task 040 §2.1's first point: no read in this file may ever `join` again --
/// the task 039 sighting showed that an immediately-awaited `join`, with
/// nothing else in between, is not protected from the eval-lifetime race
/// either.
#[test]
fn no_join_remains_in_dom_rs() {
    let source = include_str!("../dom.rs");
    assert!(
        !source.contains(".join("),
        "dom.rs must never join an eval again (task 040 section 2.1): {source}"
    );
}
