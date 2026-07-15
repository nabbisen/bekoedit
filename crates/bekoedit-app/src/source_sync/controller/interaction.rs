use bekoedit_ui_contract::source_editor::{EditorIdentity, SourceEditorId};

use super::{ControllerAction, FocusClaim, FocusResolution, SourceSyncState};

pub(super) const MAX_JAVASCRIPT_FOCUS_TOKEN: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FocusInteraction {
    pub(super) token: u64,
    target: SourceEditorId,
    fingerprint: String,
    pub(super) command_executed: bool,
    pub(super) result_document_id: Option<u64>,
}

impl SourceSyncState {
    pub fn allocate_focus_interaction(
        &mut self,
        target: SourceEditorId,
        fingerprint: String,
    ) -> Option<(u64, Option<u64>)> {
        let token = self.next_focus_token.checked_add(1)?;
        if token > MAX_JAVASCRIPT_FOCUS_TOKEN {
            return None;
        }
        let superseded = self
            .provisional_focus
            .take()
            .or_else(|| self.pending_focus.take())
            .map(|focus| focus.token);
        self.next_focus_token = token;
        self.provisional_focus = Some(FocusInteraction {
            token,
            target,
            fingerprint,
            command_executed: false,
            result_document_id: None,
        });
        Some((token, superseded))
    }

    pub fn claim_focus_interaction(
        &mut self,
        token: u64,
        resolution: FocusResolution,
    ) -> FocusClaim {
        let Some(interaction) = self.provisional_focus.take_if(|item| item.token == token) else {
            return FocusClaim::Stale;
        };
        if resolution == FocusResolution::Armed {
            self.pending_focus = Some(interaction);
        }
        FocusClaim::Claimed
    }

    pub fn cancel_focus_interactions(&mut self) -> Option<u64> {
        let provisional = self.provisional_focus.take().map(|item| item.token);
        let pending = self.pending_focus.take().map(|item| item.token);
        provisional.into_iter().chain(pending).max()
    }

    pub fn cancel_focus_token(&mut self, token: u64) -> bool {
        let mut cancelled = false;
        if self
            .provisional_focus
            .as_ref()
            .is_some_and(|item| item.token == token)
        {
            self.provisional_focus = None;
            cancelled = true;
        }
        if self
            .pending_focus
            .as_ref()
            .is_some_and(|item| item.token == token)
        {
            self.pending_focus = None;
            cancelled = true;
        }
        cancelled
    }

    pub fn active_command_focus_token(&self) -> Option<u64> {
        self.protected_focus_token.or_else(|| {
            self.waiting_command
                .as_ref()
                .and_then(|pending| pending.focus_token)
        })
    }

    pub fn focus_command_completed(
        &mut self,
        token: Option<u64>,
        success: bool,
        result_document_id: Option<u64>,
    ) -> Option<u64> {
        let token = token?;
        let interaction = self.pending_focus.as_mut()?;
        if interaction.token != token {
            return None;
        }
        if success && result_document_id.is_some() {
            interaction.command_executed = true;
            interaction.result_document_id = result_document_id;
            None
        } else {
            self.pending_focus.take().map(|item| item.token)
        }
    }

    pub(super) fn queue_ready_focus(&mut self, identity: EditorIdentity) {
        let Some(interaction) = self.pending_focus.as_ref() else {
            return;
        };
        if !interaction.command_executed
            || interaction.target != identity.editor_id
            || interaction.result_document_id != Some(identity.document_id)
        {
            return;
        }
        let interaction = self.pending_focus.take().expect("checked pending focus");
        self.actions.push(ControllerAction::Focus {
            token: interaction.token,
            identity,
            fingerprint: interaction.fingerprint,
        });
    }
}
