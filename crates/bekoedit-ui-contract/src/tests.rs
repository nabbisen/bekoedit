//! RFC-002 acceptance: payloads serialize round-trip and malformed input
//! is a recoverable error.

use std::path::PathBuf;

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
