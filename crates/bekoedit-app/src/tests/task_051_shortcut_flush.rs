//! Task 051 §2.1: `shortcuts.js` flushes a focused Form field's pending
//! text before relaying a shortcut, unless a composition is open. Split
//! out of `tests.rs` per the ELOC guideline.

/// The flush must happen before the shortcut relays, never after --
/// Rust's own commit (through Dioxus's synchronous XHR) must have
/// already landed by the time the relay message is even sent, or a
/// keyboard command could still race ahead of it. Mutated (swapped the
/// two statements) during this task: caught by name, reverted clean.
#[test]
fn the_flush_happens_before_the_shortcut_relays() {
    let source = crate::app::SHORTCUTS_SOURCE;
    let dispatch_at = source
        .find("el.dispatchEvent(new Event(\"change\"")
        .expect("the flush dispatch");
    let relay_at = source
        .find("__bk_shortcut_relay?.(")
        .expect("the shortcut relay call");
    assert!(
        dispatch_at < relay_at,
        "the flush (byte {dispatch_at}) must happen before the relay \
         (byte {relay_at}), not after"
    );
}

/// A composition in progress must never be flushed -- the dispatch is
/// reached only through `if (!composing)`. Mutated (removed the guard)
/// during this task: caught by name, reverted clean.
#[test]
fn the_flush_is_skipped_while_composing() {
    let source = crate::app::SHORTCUTS_SOURCE;
    let dispatch_at = source
        .find("el.dispatchEvent(new Event(\"change\"")
        .expect("the flush dispatch");
    let guard = source[..dispatch_at].trim_end();
    assert!(
        guard.ends_with("if (!composing) {"),
        "the flush must be guarded by `if (!composing)`: {guard}"
    );
}
