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
