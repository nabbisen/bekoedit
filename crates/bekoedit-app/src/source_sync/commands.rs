use bekoedit_core::{AppState, StoreError};
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::ToastKind;

use super::SourceCommand;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandNotice {
    pub kind: ToastKind,
    pub message: String,
}

pub fn execute(
    state: &mut AppState,
    mode: &mut EditorMode,
    settings_open: &mut bool,
    command: &SourceCommand,
    now_ms: u64,
) -> Result<Option<CommandNotice>, StoreError> {
    match command {
        SourceCommand::SwitchMode(target) => {
            *mode = *target;
            Ok(None)
        }
        SourceCommand::OpenSettings => {
            *settings_open = true;
            Ok(None)
        }
        SourceCommand::SaveNow => {
            trace_save_intent(state);
            state.save_now(now_ms)?;
            Ok(Some(notice(ToastKind::Success, "Saved")))
        }
        SourceCommand::SaveAs(path) => {
            trace_save_intent(state);
            state.save_as(path.clone(), now_ms)?;
            Ok(Some(notice(ToastKind::Success, "Saved")))
        }
        SourceCommand::OpenDocument(path) => {
            state.open_document(path)?;
            Ok(None)
        }
        SourceCommand::NewUntitled => {
            state.new_untitled();
            *mode = EditorMode::Text;
            Ok(None)
        }
        SourceCommand::OpenWorkspace(path) => {
            state.open_workspace(path, now_ms)?;
            Ok(None)
        }
        SourceCommand::CloseWorkspace => {
            state.close_workspace();
            Ok(None)
        }
        SourceCommand::RestoreHistory(entry) => {
            state.restore_history(entry, now_ms)?;
            Ok(Some(notice(ToastKind::Info, "History restored")))
        }
        SourceCommand::MoveSectionUp(index) => {
            state.move_section_up(*index, now_ms)?;
            Ok(None)
        }
        SourceCommand::MoveSectionDown(index) => {
            state.move_section_down(*index, now_ms)?;
            Ok(None)
        }
    }
}

fn notice(kind: ToastKind, message: &str) -> CommandNotice {
    CommandNotice {
        kind,
        message: message.to_string(),
    }
}

/// What [`trace_save_intent`] reports, pulled out pure (task 049 §2.2): the
/// dirty flag and canonical length when a session is open, or that none
/// is. `save_now` is a no-op whenever `dirty` is false (`store.rs`), so
/// this line is what tells "no save happened" apart from "a save without
/// the pending text" in the release-checks log.
fn describe_save_intent(state: &AppState) -> String {
    match state.session.as_ref() {
        Some(session) => format!(
            "dirty={} bytes={}",
            session.dirty,
            session.canonical_text.len()
        ),
        None => "no session open".to_string(),
    }
}

fn trace_save_intent(state: &AppState) {
    crate::bridge::trace("source.form_commit.saved", describe_save_intent(state));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bekoedit_core::DocumentSession;
    use bekoedit_fs::RecoveryStore;

    fn app() -> AppState {
        let dir = tempfile::tempdir().unwrap().keep();
        let mut app = AppState::new(
            RecoveryStore::at(dir.join(".recovery")),
            dir.join(".recent.json"),
            100,
        );
        app.new_untitled();
        app
    }

    #[test]
    fn no_open_session_is_named_plainly() {
        let mut state = app();
        state.session = None;
        assert_eq!(describe_save_intent(&state), "no session open");
    }

    #[test]
    fn a_clean_session_reports_dirty_false_with_its_own_length() {
        let mut state = app();
        let old = state.session.take().unwrap();
        state.session = Some(DocumentSession::from_text(
            old.document_id,
            old.path,
            "abc\n".into(),
        ));
        state.session.as_mut().unwrap().dirty = false;
        assert_eq!(describe_save_intent(&state), "dirty=false bytes=4");
    }

    #[test]
    fn a_dirty_session_reports_dirty_true_with_its_own_length() {
        let mut state = app();
        let old = state.session.take().unwrap();
        state.session = Some(DocumentSession::from_text(
            old.document_id,
            old.path,
            "abc def\n".into(),
        ));
        state.session.as_mut().unwrap().dirty = true;
        assert_eq!(describe_save_intent(&state), "dirty=true bytes=8");
    }
}
