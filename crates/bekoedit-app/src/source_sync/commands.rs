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
            state.save_now(now_ms)?;
            Ok(Some(notice(ToastKind::Success, "Saved")))
        }
        SourceCommand::SaveAs(path) => {
            state.save_as(path.clone(), now_ms)?;
            Ok(Some(notice(ToastKind::Success, "Saved")))
        }
        SourceCommand::OpenDocument(path) => {
            state.open_document(path)?;
            Ok(None)
        }
        SourceCommand::NewUntitled => {
            state.new_untitled();
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
