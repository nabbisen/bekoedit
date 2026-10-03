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
