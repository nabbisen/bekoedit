use super::*;

/// Task 055 §2.3: every real shortcut message arrives exactly like this --
/// `shortcuts.js` always sends `JSON.stringify(...)`, so `raw` is a
/// `Value::String`, never an object. Mutation: have the shortcut loop call
/// `serde_json::from_value` directly again (as it did before this task),
/// and this fails -- `from_value` cannot decode a string against an
/// internally tagged enum like `AppMsg`. This test goes through
/// `bridge::decode_relay_message`, the exact function the loop calls, so
/// that mutation is real.
#[test]
fn a_stringified_shortcut_message_decodes() {
    let raw = serde_json::Value::String(
        r#"{"type":"shortcut","key":"save","composing":false}"#.to_string(),
    );
    let AppMsg::Shortcut { key, composing } = bridge::decode_relay_message::<AppMsg>(raw).unwrap();
    assert_eq!(key, "save");
    assert!(!composing);
}

/// Not the shape the real page ever sends, but `decode_relay_message` must
/// not regress to handling only strings either.
#[test]
fn an_object_shortcut_message_decodes_too() {
    let raw = serde_json::json!({
        "type": "shortcut",
        "key": "mode_text",
        "composing": true,
    });
    let AppMsg::Shortcut { key, composing } = bridge::decode_relay_message::<AppMsg>(raw).unwrap();
    assert_eq!(key, "mode_text");
    assert!(composing);
}
