use super::*;

#[test]
fn lifecycle_dispatch_requires_the_acknowledged_exact_generation() {
    for script in [
        dispatch_request_js("{}", None, 41),
        dispatch_request_js("{}", Some("{}"), 41),
    ] {
        assert!(script.contains("window.__bk.dispatchForRelayGeneration(request, 41)"));
        assert!(!script.contains("__bkGeneration === 40"));
    }
}

#[test]
fn editor_bundle_is_cargo_native_and_contains_the_live_facade() {
    assert!(EDITOR_BUNDLE.starts_with("(()=>{"));
    assert!(EDITOR_BUNDLE.contains("window.__bk="));
    assert!(!EDITOR_BUNDLE.contains("This should be replaced by dx"));
}

#[test]
fn trace_diagnostics_decode_and_format_only_fixed_safe_fields() {
    let event = decode::<SourceEditorEvent>(serde_json::json!({
        "type": "trace",
        "protocolVersion": BRIDGE_SCHEMA_VERSION,
        "instanceId": 4,
        "event": "source.focus.rejected.guard",
        "focusToken": 7,
        "focusGuardDiagnostic": {
            "outcome": "rejected",
            "reason": "divertedFocusIn",
            "tokenRelation": "match",
            "diversion": "focusIn",
            "fingerprintRelation": "equal",
            "originConnection": "connected",
            "activeElementRelation": "other",
            "removalPolicy": "launchMayBeRemoved",
            "removedBodyFallback": "ineligible",
            "fingerprintValue": "forbidden-secret"
        }
    }))
    .unwrap();
    let SourceEditorEvent::Trace {
        instance_id,
        focus_token,
        focus_guard_diagnostic,
        ..
    } = event
    else {
        panic!("expected trace");
    };
    let formatted = format_trace_details(instance_id, focus_token, focus_guard_diagnostic.as_ref());
    assert_eq!(
        formatted,
        "token=7 instance_id=Some(EditorInstanceId(4)) \
         reason=divertedFocusIn outcome=rejected token_relation=match \
         diversion=focusIn fingerprint=equal origin=connected active=other \
         removal_policy=launchMayBeRemoved fallback=ineligible"
    );
    assert!(!formatted.contains("forbidden-secret"));
}

#[test]
fn legacy_non_focus_trace_remains_decodable_and_unchanged() {
    let event = decode::<SourceEditorEvent>(serde_json::json!({
        "type": "trace",
        "protocolVersion": BRIDGE_SCHEMA_VERSION,
        "instanceId": null,
        "event": "js.dispatch.bridge_error"
    }))
    .unwrap();
    let SourceEditorEvent::Trace {
        instance_id,
        focus_token,
        focus_guard_diagnostic,
        ..
    } = event
    else {
        panic!("expected trace");
    };
    assert_eq!(
        format_trace_details(instance_id, focus_token, focus_guard_diagnostic.as_ref()),
        "instance_id=None"
    );
}

// ---- Task 031: the bridge payload reaches the page as a string literal ----

/// A payload with every character the encoding has to survive: both line
/// separators, a quote, a backslash, `</script>`, and a C0 control character.
fn hard_payload() -> serde_json::Value {
    serde_json::json!({
        "type": "applyDocument",
        "text": "one\u{2028}two\u{2029}three \"quoted\" back\\slash </script> \u{1} end",
    })
}

/// The literal the script assigns to `request`, up to its closing quote,
/// checked to be exactly one JavaScript string literal: every `\` starts a
/// valid escape, and nothing that ends or breaks a literal appears raw.
fn assert_single_string_literal(script: &str, assignment: &str) {
    let start = script
        .find(assignment)
        .unwrap_or_else(|| panic!("no `{assignment}`"))
        + assignment.len();
    let literal: Vec<char> = script[start..].chars().collect();
    assert_eq!(
        literal[0], '"',
        "starts a string literal, not an object literal"
    );
    let mut index = 1;
    loop {
        match literal[index] {
            '"' => break,
            '\\' => {
                index += 1;
                match literal[index] {
                    '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' => {}
                    'u' => {
                        assert!(
                            literal[index + 1..index + 5]
                                .iter()
                                .all(char::is_ascii_hexdigit)
                        );
                        index += 4;
                    }
                    other => panic!("invalid escape \\{other}"),
                }
            }
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                panic!("a raw {c:?} inside the literal")
            }
            _ => {}
        }
        index += 1;
    }
}

#[test]
fn the_payload_is_one_string_literal_with_the_line_separators_escaped() {
    let payload = serde_json::to_string(&hard_payload()).unwrap();
    for script in [
        dispatch_request_js(&payload, None, 41),
        dispatch_request_js(&payload, Some(&payload), 41),
    ] {
        assert_single_string_literal(&script, "const request = ");
        assert!(script.contains("\\u2028") && script.contains("\\u2029"));
        assert!(!script.contains('\u{2028}') && !script.contains('\u{2029}'));
        assert!(!script.contains("const request = {"), "no object literal");
    }
}

#[test]
fn the_fallback_is_a_string_literal_too() {
    let payload = serde_json::to_string(&hard_payload()).unwrap();
    let script = dispatch_request_js("{}", Some(&payload), 41);
    assert_single_string_literal(&script, "relay(");
    assert!(
        !script.contains("JSON.stringify("),
        "the fallback is sent as the JSON text itself"
    );
}

#[test]
fn the_page_still_parses_a_string_request() {
    let editor = include_str!("../../../js/src/editor.js");
    assert!(
        editor.contains(r#"typeof request === "string" ? JSON.parse(request) : request"#),
        "editor.js's dispatch is what turns the literal back into the request"
    );
}

fn node_or_skip() -> bool {
    let present = std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok();
    if !present {
        assert!(
            std::env::var_os("CI").is_none(),
            "node must be on PATH in CI: the bridge payload check evaluates the emitted source"
        );
        eprintln!("skipped: node is not on PATH");
    }
    present
}

/// Evaluates the emitted source under node's `vm`, as the page would, and
/// fails with node's own message if the request that arrives differs.
fn evaluate_in_node(mode: &str, script: &str, expected: &serde_json::Value) {
    let dir = std::env::temp_dir().join(format!(
        "bekoedit-bridge-payload-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let script_path = dir.join("emitted.js");
    let expected_path = dir.join("expected.json");
    std::fs::write(&script_path, script).unwrap();
    std::fs::write(&expected_path, serde_json::to_string(expected).unwrap()).unwrap();
    let output = std::process::Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/js/test/support/bridge-payload-harness.mjs"
        ))
        .args([mode])
        .arg(&script_path)
        .arg(&expected_path)
        .args([&BRIDGE_SCHEMA_VERSION.to_string(), "41", SOURCE_RELAY])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        output.status.success(),
        "node rejected the emitted source ({mode}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_receives_the_hard_payload_deep_equal() {
    if !node_or_skip() {
        return;
    }
    let expected = hard_payload();
    let payload = serde_json::to_string(&expected).unwrap();
    evaluate_in_node(
        "dispatch",
        &dispatch_request_js(&payload, None, 41),
        &expected,
    );
    evaluate_in_node(
        "fallback",
        &dispatch_request_js("{}", Some(&payload), 41),
        &expected,
    );
}

#[test]
fn node_receives_a_one_mebibyte_document_deep_equal() {
    if !node_or_skip() {
        return;
    }
    let mut text = String::new();
    while text.len() < 1024 * 1024 {
        text.push_str(
            "A line of text, with \"quotes\", a \\ slash, é日本🙂, and\u{2028}a separator.\r\n",
        );
    }
    let expected = serde_json::json!({ "type": "applyDocument", "text": text });
    let payload = serde_json::to_string(&expected).unwrap();
    assert!(payload.len() > 1024 * 1024);
    evaluate_in_node(
        "dispatch",
        &dispatch_request_js(&payload, None, 41),
        &expected,
    );
}
