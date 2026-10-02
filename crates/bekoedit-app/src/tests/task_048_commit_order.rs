//! Task 048 D1: a Form Mode field's pending text must be committed before
//! any command that saves or leaves the document's current state runs --
//! ordered, not timed. Split out of `tests.rs` per the ELOC guideline.

/// The commit must be ordered before the command runs, not timed -- a
/// source-order check, the same convention
/// `bridge::tests::the_release_is_sent_only_after_the_envelope_is_received`
/// uses, since the real ordering needs a live WebView to observe.
#[test]
fn pending_form_field_is_committed_before_a_queued_command_runs() {
    let source = include_str!("../source_sync.rs");
    let commit_at = source
        .find("form_commit::commit_pending_form_field(state, mode).await;")
        .expect("the commit is awaited");
    let submit_at = source
        .find(".submit_with_focus(")
        .expect("submit_with_focus is called");
    assert!(
        commit_at < submit_at,
        "the pending-field commit (byte {commit_at}) must be awaited before \
         submit_with_focus (byte {submit_at}), not after"
    );
}

/// Autosave bypasses the command queue entirely, so it needs this same
/// ordering proven at its own call site.
#[test]
fn pending_form_field_is_committed_before_autosave_ticks() {
    let source = include_str!("../app.rs");
    let commit_at = source
        .find("crate::source_sync::commit_pending_form_field(app, autosave_mode).await;")
        .expect("the commit is awaited");
    let tick_at = source
        .find("s.autosave_tick(now_ms());")
        .expect("autosave_tick is called");
    assert!(
        commit_at < tick_at,
        "the pending-field commit (byte {commit_at}) must be awaited before \
         autosave_tick (byte {tick_at}), not after"
    );
}
