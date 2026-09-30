//! RFC-002 acceptance: payloads serialize round-trip and malformed input
//! is a recoverable error.

use crate::EditorMode;
use crate::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, EditorInstanceId, FocusGuardActiveElementRelation, FocusGuardDiagnostic,
        FocusGuardDiversion, FocusGuardFallback, FocusGuardFingerprintRelation,
        FocusGuardOriginConnection, FocusGuardOutcome, FocusGuardReason, FocusGuardRemovalPolicy,
        FocusGuardTokenRelation, OperationId, PasteFallbackReason, PasteOutcome, SourceEditorEvent,
        SourceEditorId, SourceEditorRequest, SourceEpoch,
    },
};

#[test]
fn mode_serialization_is_stable() {
    assert_eq!(
        serde_json::to_string(&EditorMode::Form).unwrap(),
        "\"form\""
    );
}

#[test]
fn source_editor_protocol_is_version_two() {
    assert_eq!(BRIDGE_SCHEMA_VERSION, 2);
    let probe = SourceEditorRequest::current_probe(OperationId::new(9));
    assert_eq!(probe.protocol_version(), 2);
}

#[test]
fn source_editor_messages_round_trip_with_camel_case_fields() {
    let identity = EditorIdentity {
        instance_id: EditorInstanceId::new(3),
        editor_id: SourceEditorId::Text,
        document_id: 4,
        epoch: SourceEpoch::new(5),
    };
    let event = SourceEditorEvent::Snapshot {
        protocol_version: BRIDGE_SCHEMA_VERSION,
        operation_id: OperationId::new(6),
        identity,
        seq: 7,
        text: "# 日本語\n".into(),
        composing: false,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"snapshot\""));
    assert!(json.contains("\"protocolVersion\":2"));
    assert!(json.contains("\"operationId\":6"));
    assert_eq!(
        serde_json::from_str::<SourceEditorEvent>(&json).unwrap(),
        event
    );
}

#[test]
fn unsupported_source_editor_event_version_is_detectable() {
    let event = SourceEditorEvent::BundleReady {
        protocol_version: 1,
        operation_id: OperationId::new(1),
    };
    assert!(!event.has_supported_version());
}

#[test]
fn focus_guard_trace_diagnostic_round_trips_with_fixed_camel_case_enums() {
    let event = SourceEditorEvent::Trace {
        protocol_version: BRIDGE_SCHEMA_VERSION,
        instance_id: Some(EditorInstanceId::new(4)),
        event: "source.focus.rejected.guard".into(),
        focus_token: Some(7),
        focus_guard_diagnostic: Some(FocusGuardDiagnostic {
            outcome: FocusGuardOutcome::Rejected,
            reason: FocusGuardReason::DivertedFocusIn,
            token_relation: FocusGuardTokenRelation::Match,
            diversion: FocusGuardDiversion::FocusIn,
            fingerprint_relation: FocusGuardFingerprintRelation::Equal,
            origin_connection: FocusGuardOriginConnection::Connected,
            active_element_relation: FocusGuardActiveElementRelation::Other,
            removal_policy: FocusGuardRemovalPolicy::LaunchMayBeRemoved,
            removed_body_fallback: FocusGuardFallback::Ineligible,
        }),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"focusToken\":7"));
    assert!(json.contains("\"reason\":\"divertedFocusIn\""));
    assert!(json.contains("\"activeElementRelation\":\"other\""));
    assert_eq!(
        serde_json::from_str::<SourceEditorEvent>(&json).unwrap(),
        event
    );
}

#[test]
fn legacy_trace_without_focus_diagnostic_remains_decodable() {
    let event = serde_json::from_str::<SourceEditorEvent>(
        r#"{"type":"trace","protocolVersion":2,"instanceId":null,"event":"legacy"}"#,
    )
    .unwrap();
    assert_eq!(
        event,
        SourceEditorEvent::Trace {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            instance_id: None,
            event: "legacy".into(),
            focus_token: None,
            focus_guard_diagnostic: None,
        }
    );
}

// ---- RFC-046: the paste request/reply pair ---------------------------------

fn identity() -> EditorIdentity {
    EditorIdentity {
        instance_id: EditorInstanceId::new(3),
        editor_id: SourceEditorId::Text,
        document_id: 4,
        epoch: SourceEpoch::new(5),
    }
}

#[test]
fn paste_requested_round_trips_with_html_present() {
    let event = SourceEditorEvent::PasteRequested {
        protocol_version: BRIDGE_SCHEMA_VERSION,
        identity: identity(),
        token: 11,
        html: Some("<p>日本語</p>".into()),
        plain_length: 4,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"pasteRequested\""));
    assert!(json.contains("\"plainLength\":4"));
    assert!(json.contains("\"html\":\"<p>日本語</p>\""));
    assert_eq!(
        serde_json::from_str::<SourceEditorEvent>(&json).unwrap(),
        event
    );
    assert_eq!(event.protocol_version(), BRIDGE_SCHEMA_VERSION);
}

#[test]
fn paste_requested_round_trips_with_no_html_decided_locally() {
    let event = SourceEditorEvent::PasteRequested {
        protocol_version: BRIDGE_SCHEMA_VERSION,
        identity: identity(),
        token: 12,
        html: None,
        plain_length: 40,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"html\":null"));
    assert_eq!(
        serde_json::from_str::<SourceEditorEvent>(&json).unwrap(),
        event
    );
}

#[test]
fn paste_discarded_round_trips() {
    let event = SourceEditorEvent::PasteDiscarded {
        protocol_version: BRIDGE_SCHEMA_VERSION,
        identity: identity(),
        token: 14,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"pasteDiscarded\""));
    assert_eq!(
        serde_json::from_str::<SourceEditorEvent>(&json).unwrap(),
        event
    );
    assert_eq!(event.protocol_version(), BRIDGE_SCHEMA_VERSION);
}

#[test]
fn paste_result_round_trips_for_each_outcome() {
    let cases = [
        PasteOutcome::Converted {
            markdown: "# 日本語\n".into(),
            table_no_gfm_form: false,
        },
        PasteOutcome::Converted {
            markdown: "Name\n\nAlice\n".into(),
            table_no_gfm_form: true,
        },
        PasteOutcome::Fallback {
            reason: PasteFallbackReason::TooLarge,
        },
        PasteOutcome::Fallback {
            reason: PasteFallbackReason::Failed,
        },
        PasteOutcome::Fallback {
            reason: PasteFallbackReason::TimedOut,
        },
        PasteOutcome::Empty,
    ];
    for outcome in cases {
        let request = SourceEditorRequest::PasteResult {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            identity: identity(),
            token: 13,
            outcome: outcome.clone(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"type\":\"pasteResult\""), "{json}");
        assert_eq!(
            serde_json::from_str::<SourceEditorRequest>(&json).unwrap(),
            request,
            "{outcome:?}"
        );
        assert_eq!(request.protocol_version(), BRIDGE_SCHEMA_VERSION);
    }
}

#[test]
fn paste_outcome_tags_are_the_expected_camel_case_names() {
    assert_eq!(
        serde_json::to_string(&PasteOutcome::Empty).unwrap(),
        r#"{"kind":"empty"}"#
    );
    assert_eq!(
        serde_json::to_string(&PasteOutcome::Fallback {
            reason: PasteFallbackReason::TimedOut
        })
        .unwrap(),
        r#"{"kind":"fallback","reason":"timedOut"}"#
    );
    assert_eq!(
        serde_json::to_string(&PasteOutcome::Converted {
            markdown: "x".into(),
            table_no_gfm_form: true,
        })
        .unwrap(),
        r#"{"kind":"converted","markdown":"x","tableNoGfmForm":true}"#
    );
}
