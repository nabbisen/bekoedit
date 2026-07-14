//! RFC-002 acceptance: payloads serialize round-trip and malformed input
//! is a recoverable error.

use std::path::PathBuf;

use crate::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, EditorInstanceId, OperationId, SourceEditorEvent, SourceEditorId,
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
