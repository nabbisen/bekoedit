//! Unit tests for the focus layer -- pure helpers and the task 022
//! consistency checks; the async guard flow needs a live WebView and is
//! covered by the RFC-044 harness.

use super::*;

const ALL_MODES: [EditorMode; 4] = [
    EditorMode::Text,
    EditorMode::Split,
    EditorMode::Preview,
    EditorMode::Form,
];

fn claim_for(mode: EditorMode) -> Option<SourceEditorId> {
    match mode {
        EditorMode::Text => Some(SourceEditorId::Text),
        EditorMode::Split => Some(SourceEditorId::Split),
        EditorMode::Preview | EditorMode::Form => None,
    }
}

#[test]
fn new_untitled_always_targets_the_text_editor() {
    for current in ALL_MODES {
        assert_eq!(
            focus_target(&SourceCommand::NewUntitled, current),
            Some(SourceEditorId::Text),
            "from {current:?}"
        );
    }
}

#[test]
fn open_document_claims_focus_for_the_current_mode() {
    // Task 014 §3's table, one row per mode.
    let open = SourceCommand::OpenDocument("notes/a.md".into());
    assert_eq!(
        focus_target(&open, EditorMode::Text),
        Some(SourceEditorId::Text)
    );
    assert_eq!(
        focus_target(&open, EditorMode::Split),
        Some(SourceEditorId::Split)
    );
    assert_eq!(focus_target(&open, EditorMode::Preview), None);
    assert_eq!(focus_target(&open, EditorMode::Form), None);
}

#[test]
fn switch_mode_claims_its_target_regardless_of_the_current_mode() {
    for current in ALL_MODES {
        for target in ALL_MODES {
            assert_eq!(
                focus_target(&SourceCommand::SwitchMode(target), current),
                claim_for(target),
                "{current:?} -> {target:?}"
            );
        }
    }
}

mod task_022;

#[test]
fn commands_without_an_editor_destination_claim_nothing() {
    for current in ALL_MODES {
        for command in [
            SourceCommand::OpenSettings,
            SourceCommand::SaveNow,
            SourceCommand::CloseWorkspace,
        ] {
            assert_eq!(focus_target(&command, current), None, "{command:?}");
        }
    }
}

#[test]
fn document_link_launch_ids_are_namespaced_and_position_unique() {
    let tree = SourceInteractionOrigin::tree_row(Path::new("notes/a.md"));
    assert_eq!(tree.launch_id(), Some("tree:notes/a.md"));
    assert_eq!(tree.invocation, "pointer");
    assert_eq!(tree.removal_policy, "launchMayBeRemoved");

    // Two links from one file on one line differ only by position.
    let first = SourceInteractionOrigin::backlink(0, Path::new("b.md"), 3);
    let second = SourceInteractionOrigin::backlink(1, Path::new("b.md"), 3);
    assert_eq!(first.launch_id(), Some("backlink:0:b.md:3"));
    assert_ne!(first.launch_id(), second.launch_id());
    assert_eq!(first.invocation, "pointer");
    assert_eq!(first.removal_policy, "launchMayBeRemoved");

    let search = SourceInteractionOrigin::search_result(2, Path::new("sub/child.md"), 1);
    assert_eq!(search.launch_id(), Some("search:2:sub/child.md:1"));
    assert_eq!(search.removal_policy, "launchMayBeRemoved");

    // Fixed ids keep their borrowed, unprefixed form.
    let fixed = SourceInteractionOrigin::start_control("start-new");
    assert_eq!(fixed.launch_id(), Some("start-new"));
    for id in [tree.launch_id(), first.launch_id()] {
        assert_ne!(id, fixed.launch_id());
    }
}

#[test]
fn tree_and_backlink_opens_launch_a_focus_interaction() {
    // Task 014 §5.4: both call sites reach `focus_target` through
    // `submit_source_interaction` with their namespaced launch hook, rather
    // than the plain submit that cancels any focus claim.
    let tree_row = include_str!("../../components/explorer/tree_row.rs");
    let backlinks = include_str!("../../components/backlinks_panel.rs");
    for (name, source, origin) in [
        ("tree_row", tree_row, "SourceInteractionOrigin::tree_row("),
        ("backlinks", backlinks, "SourceInteractionOrigin::backlink("),
    ] {
        assert!(source.contains("submit_source_interaction("), "{name}");
        assert!(!source.contains("submit_source_command("), "{name}");
        assert!(source.contains(origin), "{name}");
        assert!(source.contains("\"data-source-focus-launch\":"), "{name}");
    }
}

#[test]
fn guard_acknowledgement_decodes_from_the_javascript_string_payload() {
    let acknowledgement = decode_guard_acknowledgement(r#"{"token":7,"armed":true,"reason":null}"#)
        .expect("valid acknowledgement");

    assert_eq!(acknowledgement.token, 7);
    assert!(acknowledgement.armed);
    assert_eq!(acknowledgement.reason, None);
}

#[test]
fn eager_guard_bundle_owns_arm_and_cancel_before_editor_bootstrap() {
    assert!(FOCUS_GUARD_BOOTSTRAP.contains("__bkFocusGuards"));
    assert!(FOCUS_GUARD_BOOTSTRAP.contains("protocolVersion"));
    assert!(FOCUS_GUARD_BOOTSTRAP.contains("consumeDiagnostic"));
    assert!(!FOCUS_GUARD_BOOTSTRAP.contains("CodeMirror"));
    assert_eq!(FOCUS_GUARD_PROTOCOL_VERSION, 2);
}

// ---- Task 031: the focus scripts carry their JSON as string literals ------

#[test]
fn the_arm_script_parses_its_request_from_a_string_literal() {
    let request = serde_json::json!({ "token": 3, "launchId": "tree:a\u{2028}b\u{2029}c\"d\\e" });
    let script = arm_focus_guard_js(&serde_json::to_string(&request).unwrap());
    assert!(script.contains("const request = JSON.parse(\""), "{script}");
    assert!(script.contains("\\u2028") && script.contains("\\u2029"));
    assert!(!script.contains('\u{2028}') && !script.contains('\u{2029}'));
    assert!(!script.contains("const request = {"));
}

// ---- Task 039: the acknowledgement must outlive the page's garbage
// collector --------------------------------------------------------------

#[test]
fn each_arm_failure_gets_its_own_trace_and_none_of_them_is_the_timeout_event() {
    let cases = [
        (
            ArmFailure::Unanswered("EvalError::Finished - eval has already ran".to_string()),
            "source.focus.guard.unanswered",
        ),
        (
            ArmFailure::Undecodable("not json".to_string()),
            "source.focus.guard.undecodable",
        ),
        (
            ArmFailure::Unencodable("recursion limit".to_string()),
            "source.focus.guard.unencodable",
        ),
    ];
    for (failure, expected_event) in cases {
        let (event, detail) = arm_failure_trace(&failure);
        assert_eq!(event, expected_event, "{failure:?}");
        assert_ne!(
            event, "source.focus.guard.timeout",
            "only an elapsed ARM_TIMEOUT may use this event, at the call site"
        );
        let expected_detail = match &failure {
            ArmFailure::Unanswered(detail)
            | ArmFailure::Undecodable(detail)
            | ArmFailure::Unencodable(detail) => detail,
        };
        assert_eq!(detail, expected_detail);
    }
}

#[test]
fn the_arm_script_returns_its_promise_and_waits_for_release_with_a_bound() {
    let script = arm_focus_guard_js("{}");
    // The leading `return` is the fix itself (task 039 section 2.2): without
    // it, the outer evaluated function resolves right after `dioxus.send`,
    // regardless of what the inner IIFE goes on to do.
    assert!(
        script.contains("return (async () => {"),
        "the evaluated function must return its promise: {script}"
    );
    assert!(
        script.contains("dioxus.send(JSON.stringify(ack))"),
        "{script}"
    );
    // Waits for Rust's release, but bounded -- a string-level check, as for
    // `render_locate_script` (task 023 section 5.4).
    assert!(script.contains("dioxus.recv()"), "{script}");
    assert!(
        script.contains(&format!("setTimeout(resolve, {GUARD_RELEASE_TIMEOUT_MS})")),
        "{script}"
    );
    assert!(
        GUARD_RELEASE_TIMEOUT_MS > ARM_TIMEOUT.as_millis() as u64,
        "the page's own bound must outlast ARM_TIMEOUT, or the ordinary path \
         could resolve via the bound instead of the release"
    );
}

#[test]
fn the_release_is_sent_only_after_the_acknowledgement_is_received() {
    // Task 039 section 2.2's third point: Rust sends the release after its
    // `recv` returns. `arm_focus_guard` itself needs a live WebView to run
    // (the async guard flow, per this module's own doc comment), so this is
    // a source-order check, not a call-order one -- the project's existing
    // convention for a JS/async ordering claim it cannot otherwise observe
    // headlessly (e.g. `application_root_assets_are_cargo_native...`-style
    // `include_str!` checks elsewhere in this crate).
    let source = include_str!("../focus.rs");
    let recv_at = source
        .find(".recv::<String>()")
        .expect("the acknowledgement is read with eval.recv::<String>()");
    let send_at = source
        .find("let _ = eval.send(true);")
        .expect("the release is sent with eval.send(true)");
    assert!(
        recv_at < send_at,
        "the release (byte {send_at}) must be sent after the acknowledgement \
         is received (byte {recv_at}), not before"
    );
}

#[test]
fn the_consume_script_parses_its_identity_and_quotes_its_fingerprint() {
    let identity = serde_json::to_string(&serde_json::json!({ "epoch": 2 })).unwrap();
    let script = consume_focus_guard_js(7, &identity, "fp\u{2028}\"\\");
    assert!(script.contains("identity: JSON.parse(\""), "{script}");
    assert!(
        script.contains("fingerprint: \"fp\\u2028\\\"\\\\\""),
        "{script}"
    );
    assert!(!script.contains('\u{2028}'));
}
