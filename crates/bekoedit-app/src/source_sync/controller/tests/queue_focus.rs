//! Task 022 §6.4: a claim that proceeds without focus (RFC-042's guard
//! divert) must not block the command it was armed for, and must not force
//! focus once the command runs. This is unaffected by task 022's own fix --
//! it is the existing `FocusResolution::ProceedWithoutFocus` path, exercised
//! through case A's own scenario (a switch accepted while a different one is
//! in flight) rather than the ordinary Ready case `proceed_without_focus_clears_eligibility`
//! already covers.

use super::*;

fn switch(mode: EditorMode) -> SourceCommand {
    SourceCommand::SwitchMode(mode)
}

#[test]
fn case_a_a_diverted_guard_still_lets_the_switch_run_but_forces_no_focus() {
    let mut app = app();
    let mut sync = SourceSyncState::default();
    make_ready(&mut sync, &mut app);
    let document_id = app.session.as_ref().unwrap().document_id;
    let SubmitOutcome::SnapshotRequested(_) =
        sync.submit(switch(EditorMode::Preview), Some(document_id), 10)
    else {
        unreachable!()
    };
    sync.drain_actions();

    // Case A: a click on Text arms a claim (proven in `focus.rs`'s own
    // tests); here the guard diverts -- focus moved elsewhere before it could
    // arm -- so the claim proceeds without focus rather than being refused.
    let (token, _) = sync
        .allocate_focus_interaction(SourceEditorId::Text, "mode-text".into())
        .unwrap();
    assert_eq!(
        sync.claim_focus_interaction(token, FocusResolution::ProceedWithoutFocus),
        FocusClaim::Claimed
    );

    // The command still runs: it is accepted into the queue, carrying the
    // diverted token, exactly as an armed one would.
    let outcome =
        sync.submit_with_focus(switch(EditorMode::Text), Some(document_id), 11, Some(token));
    assert_eq!(
        outcome,
        SubmitOutcome::Queued,
        "the divert does not block the command"
    );

    // Focus is not forced: `ProceedWithoutFocus` never records the
    // interaction as eligible, so completing the command -- successfully,
    // for this identity -- produces no `Focus` action for this token.
    let _ = sync.focus_command_completed(Some(token), true, Some(document_id));
    assert_eq!(
        sync.cancel_focus_interactions(),
        None,
        "no focus interaction is left pending to force"
    );
    assert!(
        !sync
            .drain_actions()
            .iter()
            .any(|action| matches!(action, ControllerAction::Focus { .. })),
        "a diverted claim must never produce a Focus action"
    );
}
