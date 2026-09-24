//! A small module of its own so `tests.rs` stays under the ELOC gate (task 025
//! review §3.1): a failure after the acknowledgement travels in the
//! completion, and `validate_completion` puts its reason in the rejection.

use crate::webview_smoke::transport::{
    MessageKind, PhaseCompletion, SMOKE_PROTOCOL_VERSION, validate_completion,
};

fn completion(error: Option<&str>) -> PhaseCompletion {
    PhaseCompletion {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id: 7,
        phase: "editor".into(),
        kind: MessageKind::Progress,
        acknowledgement_processed: true,
        evaluator_pinned: true,
        error: error.map(str::to_string),
    }
}

#[test]
fn a_completion_failure_s_reason_is_in_the_rejection() {
    let mut failed = completion(Some("invalid phase acknowledgement"));
    failed.acknowledgement_processed = false;
    failed.evaluator_pinned = false;
    let error = validate_completion(&failed, 7, "editor", MessageKind::Progress).unwrap_err();
    assert!(error.contains("invalid phase acknowledgement"), "{error}");
}

#[test]
fn a_completion_carrying_an_error_never_passes() {
    let odd = completion(Some("smoke evaluator pin was already occupied"));
    assert!(validate_completion(&odd, 7, "editor", MessageKind::Progress).is_err());
    assert!(validate_completion(&completion(None), 7, "editor", MessageKind::Progress).is_ok());
}

#[test]
fn a_returned_failure_deserializes_and_an_old_shape_still_does() {
    let failed: PhaseCompletion = serde_json::from_str(
        r#"{"protocolVersion":2,"exchangeId":7,"phase":"editor","kind":"progress",
            "acknowledgementProcessed":false,"evaluatorPinned":false,
            "error":"invalid phase acknowledgement"}"#,
    )
    .unwrap();
    assert_eq!(
        failed.error.as_deref(),
        Some("invalid phase acknowledgement")
    );
    let ok: PhaseCompletion = serde_json::from_str(
        r#"{"protocolVersion":2,"exchangeId":7,"phase":"editor","kind":"progress",
            "acknowledgementProcessed":true,"evaluatorPinned":true}"#,
    )
    .unwrap();
    assert_eq!(ok.error, None);
}
