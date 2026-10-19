//! RFC-049 slice 2: the shortcut and the buttons reach the store's
//! `undo_form` / `redo_form`, and a refusal raises its notice. These read the
//! wiring from other files' source, as `task_056_form_edit_refusals.rs` does,
//! and live in a file of their own so that no needle matches its own text.

const APP: &str = include_str!("../app.rs");
const HISTORY: &str = include_str!("../components/form_mode/history.rs");
const FORM_MODE: &str = include_str!("../components/form_mode.rs");
const SHORTCUTS: &str = include_str!("../../assets/shortcuts.js");

/// Mutation: drop the `form_undo` / `form_redo` branch from `app.rs` and
/// Ctrl+Z does nothing in the app.
#[test]
fn the_shortcut_relay_reaches_the_history_step_in_form_mode_only() {
    let branch = APP
        .find("key == \"form_undo\" || key == \"form_redo\"")
        .unwrap();
    let rest = &APP[branch..];
    let end = rest.find("match shortcut_command(&key) {").unwrap();
    let block = &rest[..end];
    assert!(
        block.contains("*mode_sig.read() == EditorMode::Form"),
        "{block}"
    );
    assert!(block.contains("run_history_step("), "{block}");
    assert!(
        block.contains("key == \"form_undo\""),
        "undo is chosen by the key: {block}"
    );
}

#[test]
fn the_buttons_call_undo_and_redo() {
    assert!(HISTORY.contains("run_history_step(state, toasts, lang, true)"));
    assert!(HISTORY.contains("run_history_step(state, toasts, lang, false)"));
    assert!(HISTORY.contains("state.write().undo_form(now_ms())"));
    assert!(HISTORY.contains("state.write().redo_form(now_ms())"));
    assert!(HISTORY.contains("disabled: !can_undo"));
    assert!(HISTORY.contains("disabled: !can_redo"));
}

/// Mutation: remove the `push_toast` from `run_history_step` and a refused
/// Undo is silent again.
#[test]
fn a_refused_step_is_traced_and_raises_a_notice() {
    assert!(HISTORY.contains("crate::bridge::trace(\"form.history_refused\", details)"));
    assert!(HISTORY.contains("push_toast(&mut toasts, ToastKind::Warning, tr(lang, toast_key))"));
}

#[test]
fn the_form_view_shows_the_buttons() {
    assert!(FORM_MODE.contains("FormHistoryBar { lang }"));
}

/// Mutation: remove the field check from `shortcuts.js` and Ctrl+Z in a field
/// would be taken from the browser (also caught by the JS tests and by the
/// `form_undo_redo` scenario's step 4).
#[test]
fn shortcuts_js_leaves_text_fields_to_the_browser() {
    assert!(SHORTCUTS.contains("if (isTextField(document.activeElement)) return null;"));
    assert!(SHORTCUTS.contains("if (!document.querySelector(\".form-mode\")) return null;"));
}
