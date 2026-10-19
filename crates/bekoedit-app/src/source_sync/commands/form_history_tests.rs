//! RFC-049 slice 2 §2.4: leaving Form Mode, by any route, empties the Form
//! Undo and Redo history. Every mode switch (the tabs, Ctrl+1/3/4, Split) is a
//! `SourceCommand::SwitchMode` run through `execute`.

use bekoedit_core::{AppState, DocumentSession};
use bekoedit_fs::RecoveryStore;
use bekoedit_markdown::{FormBlockEdit, FormEditCommand};
use bekoedit_ui_contract::EditorMode;

use super::*;

fn state_with_history() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let mut state = AppState::new(
        RecoveryStore::at(dir.path().join(".recovery")),
        dir.path().join(".recent.json"),
        1500,
    );
    state.session = Some(DocumentSession::from_text(
        1,
        dir.path().join("a.md"),
        "# Title\n\npara\n".into(),
    ));
    let session = state.session.as_ref().unwrap();
    let command = FormEditCommand {
        base_revision: session.revision,
        block_id: session.index.blocks[1].block_id,
        client_block_fingerprint: None,
        edit: FormBlockEdit::ReplacePlainText {
            text: "edited".into(),
        },
    };
    state.edit_form(&command, 0).unwrap();
    state.undo_form(0).unwrap();
    assert!(state.session.as_ref().unwrap().can_redo_form());
    (dir, state)
}

fn switch(state: &mut AppState, from: EditorMode, to: EditorMode) {
    let mut mode = from;
    let mut settings_open = false;
    execute(
        state,
        &mut mode,
        &mut settings_open,
        &SourceCommand::SwitchMode(to),
        1,
    )
    .unwrap();
    assert_eq!(mode, to);
}

fn history(state: &AppState) -> (bool, bool) {
    let session = state.session.as_ref().unwrap();
    (session.can_undo_form(), session.can_redo_form())
}

#[test]
fn leaving_form_mode_by_each_route_clears_the_history() {
    for to in [EditorMode::Text, EditorMode::Preview, EditorMode::Split] {
        let (_dir, mut state) = state_with_history();
        switch(&mut state, EditorMode::Form, to);
        assert_eq!(history(&state), (false, false), "Form -> {to:?}");
    }
}

#[test]
fn staying_in_form_mode_keeps_the_history() {
    let (_dir, mut state) = state_with_history();
    switch(&mut state, EditorMode::Form, EditorMode::Form);
    assert_eq!(history(&state), (false, true));
}

#[test]
fn entering_form_mode_from_another_mode_does_not_touch_the_history() {
    for from in [EditorMode::Text, EditorMode::Preview, EditorMode::Split] {
        let (_dir, mut state) = state_with_history();
        switch(&mut state, from, EditorMode::Form);
        assert_eq!(history(&state), (false, true), "{from:?} -> Form");
    }
}
