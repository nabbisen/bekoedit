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

/// Review §2.1: autosave does **not** commit a Form field's pending
/// text. Autosave writes only the document's already-committed text; the
/// pending text stays in the field, and the next blur or command commits
/// it then, so autosave loses nothing by leaving it alone. Committing on
/// every tick (`app.rs`'s background loop runs every `TICK_MS`, whether
/// or not a save is even due) was itself the per-keystroke commit §3
/// prohibits, by a second route: while a user types, it would re-render
/// the field's value out from under them roughly twice a second.
#[test]
fn the_background_tick_never_commits_a_pending_form_field() {
    let source = include_str!("../app.rs");
    assert!(
        !source.contains("commit_pending_form_field"),
        "app.rs's background tick must never call commit_pending_form_field: {source}"
    );
}
