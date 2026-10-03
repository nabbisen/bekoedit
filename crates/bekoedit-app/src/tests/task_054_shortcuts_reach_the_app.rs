//! Task 054: keyboard shortcuts never reached Rust at all --
//! `shortcuts.js` relayed only `if (window.dioxus)`, which nothing has
//! ever defined. Split out of `tests.rs` per the ELOC guideline.

/// §2.1: the relay must never be guarded by `window.dioxus`. Mutation:
/// restore `if (window.dioxus) { ... }` around the relay call, and this
/// fails.
#[test]
fn the_relay_is_never_guarded_by_window_dioxus() {
    let source = crate::app::SHORTCUTS_SOURCE;
    assert!(
        !source.contains("window.dioxus"),
        "shortcuts.js must not gate the relay on window.dioxus, which \
         nothing defines: {source}"
    );
}

/// §2.3: `toggle_explorer` is not a source command -- it saves or
/// leaves nothing, so the flush (which exists only to commit a Form
/// field before a command that could otherwise lose it) must be
/// skipped for it entirely, before the flush's own composing check.
#[test]
fn toggle_explorer_is_exempt_from_the_flush() {
    let source = crate::app::SHORTCUTS_SOURCE;
    let dispatch_at = source
        .find("el.dispatchEvent(new Event(\"change\"")
        .expect("the flush dispatch");
    let exemption_at = source
        .find("if (key !== \"toggle_explorer\")")
        .expect("the toggle_explorer exemption");
    assert!(
        exemption_at < dispatch_at,
        "toggle_explorer's exemption (byte {exemption_at}) must wrap the \
         flush dispatch (byte {dispatch_at}), not follow it"
    );
}

/// §2.3: `toggle_explorer` must never reach the composing refusal or the
/// focus machinery either -- both live behind `match shortcut_command(&key)`,
/// which only runs for every *other* key; `toggle_explorer` is handled
/// in its own branch, reached first.
#[test]
fn toggle_explorer_bypasses_the_composing_refusal_and_focus_machinery() {
    let source = include_str!("../app.rs");
    let branch_at = source
        .find("if key == \"toggle_explorer\" {")
        .expect("the toggle_explorer branch");
    let match_at = source
        .find("match shortcut_command(&key) {")
        .expect("the source-command match");
    assert!(
        branch_at < match_at,
        "toggle_explorer must be handled before shortcut_command's own \
         match, not from inside it"
    );
}
