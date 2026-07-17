//! RFC-002 acceptance: payloads serialize round-trip and malformed input
//! is a recoverable error.

use std::path::PathBuf;

use crate::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, EditorInstanceId, FocusGuardActiveElementRelation, FocusGuardDiagnostic,
        FocusGuardDiversion, FocusGuardFallback, FocusGuardFingerprintRelation,
        FocusGuardOriginConnection, FocusGuardOutcome, FocusGuardReason, FocusGuardRemovalPolicy,
        FocusGuardTokenRelation, OperationId, SourceEditorEvent, SourceEditorId,
        SourceEditorRequest, SourceEpoch,
    },
};
use crate::{EditorMode, UiToCoreCommand};

#[test]
fn commands_round_trip_through_json() {
    let cmd = UiToCoreCommand::TextSnapshot {
        document_id: 3,
        base_revision: 9,
        text: "# 日本語\n".into(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"text_snapshot\""));
    assert_eq!(serde_json::from_str::<UiToCoreCommand>(&json).unwrap(), cmd);
}

#[test]
fn malformed_payload_is_a_recoverable_error_not_a_panic() {
    let result = serde_json::from_str::<UiToCoreCommand>("{\"command\":\"nope\"}");
    assert!(result.is_err());
}

#[test]
fn mode_serialization_is_stable() {
    assert_eq!(
        serde_json::to_string(&EditorMode::Form).unwrap(),
        "\"form\""
    );
}

#[test]
fn open_workspace_path_round_trips() {
    let cmd = UiToCoreCommand::OpenWorkspace {
        path: PathBuf::from("/home/user/notes"),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert_eq!(serde_json::from_str::<UiToCoreCommand>(&json).unwrap(), cmd);
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
