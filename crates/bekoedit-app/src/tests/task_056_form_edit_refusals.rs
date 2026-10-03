//! Task 056: a refused Form edit used to vanish silently (`dispatch`'s
//! own `let _ = state.write().edit_form(&cmd, now_ms());`), against
//! RFC-047's rule that a user command is run or reported, never
//! dropped. Split out of `form_mode.rs`'s own test module (not that
//! module itself, per the ELOC guideline): a `!source.contains(...)`
//! check against `form_mode.rs`'s *own* `include_str!` would trivially
//! pass no matter what the real code does, since the needle text would
//! also appear, verbatim, inside the test's own source -- the same
//! file it is reading.

/// Mutations: go back to discarding `edit_form`'s own result with an
/// underscore binding (task 056 §4's "let _ restored"), or remove just
/// the `push_toast` call (§4's "the toast removed"), and this fails
/// either way.
#[test]
fn a_refused_edit_is_never_silently_discarded() {
    let source = include_str!("../components/form_mode.rs");
    assert!(
        !source.contains("let _ = state.write().edit_form"),
        "a refused edit must never be discarded with an underscore binding again"
    );
    assert!(
        source.contains("crate::bridge::trace(\"form.edit_refused\""),
        "dispatch must trace a refused edit"
    );
    assert!(
        source.contains("push_toast(&mut toasts, ToastKind::Warning, tr(lang, toast_key))"),
        "dispatch must show a toast naming why a refused edit did not apply"
    );
}
