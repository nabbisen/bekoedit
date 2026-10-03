use super::*;

// `eval_body`'s own envelope/decode/lifetime tests moved to `bridge.rs`
// (task 047 Part B) along with the function itself; these two stay here
// since they are specific to this module's own error-naming.

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

fn is_balanced(script: &str) -> bool {
    let mut parens = 0i32;
    let mut braces = 0i32;
    for c in script.chars() {
        match c {
            '(' => parens += 1,
            ')' => parens -= 1,
            '{' => braces += 1,
            '}' => braces -= 1,
            _ => {}
        }
        if parens < 0 || braces < 0 {
            return false;
        }
    }
    parens == 0 && braces == 0
}

#[test]
fn active_element_js_is_balanced() {
    assert!(is_balanced(ACTIVE_ELEMENT_JS), "{ACTIVE_ELEMENT_JS}");
}

/// `save_pending_field`'s focus-thief report must never read or compare
/// field content, only what identifies the element with focus.
#[test]
fn active_element_js_never_reads_value_or_text() {
    for forbidden in ["el.value", "textContent", "innerText"] {
        assert!(
            !ACTIVE_ELEMENT_JS.contains(forbidden),
            "{ACTIVE_ELEMENT_JS} contains {forbidden:?}"
        );
    }
}

fn describe(tag: &str, classes: &[&str], in_form_mode: bool) -> ActiveElementDescription {
    ActiveElementDescription {
        tag: tag.to_string(),
        id: "fb-0-1".to_string(),
        classes: classes.iter().map(|c| c.to_string()).collect(),
        in_form_mode,
    }
}

/// Task 052 §2.2: the paragraph field is recognised by tag/class/region,
/// never by its own `id` -- which changes with the field's content, so
/// a check pinned to one exact id would fail the moment the field is
/// committed to, the thing this very assertion runs right after.
#[test]
fn is_paragraph_field_ignores_the_id() {
    assert!(describe("textarea", &["paragraph-input"], true).is_paragraph_field());
    assert!(
        describe("textarea", &["paragraph-input", "blockquote-input"], true).is_paragraph_field()
    );
}

#[test]
fn is_paragraph_field_rejects_the_wrong_tag_class_or_region() {
    assert!(!describe("input", &["paragraph-input"], true).is_paragraph_field());
    assert!(!describe("textarea", &["code-input"], true).is_paragraph_field());
    assert!(!describe("textarea", &["paragraph-input"], false).is_paragraph_field());
}
