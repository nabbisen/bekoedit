use bekoedit_core::AppState;
use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEditorEvent, SourceEditorId,
};

use super::lifecycle::{
    CommandDisposition, LifecycleEffect, LifecycleReducer, LifecycleState, MountIntent,
    SessionFingerprint, TransitionError,
};
use super::{SourceCommand, SourceSyncError};

const MAX_JAVASCRIPT_FOCUS_TOKEN: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerAction {
    Lifecycle(LifecycleEffect),
    Execute {
        command: SourceCommand,
        protected: bool,
        focus_token: Option<u64>,
    },
    Focus {
        token: u64,
        identity: EditorIdentity,
        fingerprint: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusResolution {
    Armed,
    ProceedWithoutFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusClaim {
    Claimed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusInteraction {
    token: u64,
    target: SourceEditorId,
    fingerprint: String,
    command_executed: bool,
    result_document_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingCommand {
    command: SourceCommand,
    focus_token: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountOutcome {
    Started,
    AlreadyCurrent,
    Queued,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorMountHandle {
    pub instance_id: EditorInstanceId,
    pub editor_id: SourceEditorId,
    pub document_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    NoOp,
    ExecuteQueued,
    SnapshotRequested(OperationId),
    WaitingForReady,
    Busy,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    Applied,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickOutcome {
    Idle,
    TimedOut,
    TakeoverStarted,
}

#[derive(Debug, Default)]
pub struct SourceSyncState {
    pub lifecycle: LifecycleReducer,
    actions: Vec<ControllerAction>,
    waiting_command: Option<PendingCommand>,
    protected_focus_token: Option<u64>,
    bundle_probe_started: bool,
    expected_relay_generation: Option<u64>,
    relay_generation: Option<u64>,
    next_focus_token: u64,
    provisional_focus: Option<FocusInteraction>,
    pending_focus: Option<FocusInteraction>,
}

impl SourceSyncState {
    pub fn start_bundle_probe(&mut self, now_ms: u64) {
        if self.bundle_probe_started {
            return;
        }
        self.bundle_probe_started = true;
        let operation_id = self.lifecycle.begin_bundle_probe(now_ms);
        self.actions
            .push(ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(
                operation_id,
            )));
    }

    pub fn mount(&mut self, intent: MountIntent, now_ms: u64) -> MountOutcome {
        if self.matches_current_mount(&intent) {
            return MountOutcome::AlreadyCurrent;
        }
        if matches!(
            self.lifecycle.state,
            LifecycleState::Unavailable { retired: None }
        ) {
            let operation_id = self.lifecycle.begin_bundle_probe(now_ms);
            self.actions
                .push(ControllerAction::Lifecycle(LifecycleEffect::ProbeBundle(
                    operation_id,
                )));
        }
        let accepted_wait = matches!(
            self.lifecycle.state,
            LifecycleState::Unmounting { waiting: None, .. }
        );
        match self.lifecycle.begin_mount(intent, now_ms) {
            Ok(effect) => {
                self.push_effect(effect);
                MountOutcome::Started
            }
            Err(TransitionError::Busy) if accepted_wait => MountOutcome::Queued,
            Err(_) => MountOutcome::Busy,
        }
    }

    pub fn unmount(&mut self, handle: EditorMountHandle, now_ms: u64) {
        if !self.owns_handle(handle) {
            return;
        }
        self.force_unmount(now_ms);
    }

    pub fn force_unmount(&mut self, now_ms: u64) {
        self.waiting_command = None;
        self.protected_focus_token = None;
        if let Ok(Some(effect)) = self.lifecycle.begin_unmount(now_ms) {
            self.push_effect(effect);
        }
    }

    pub fn shutdown(&mut self, now_ms: u64) -> Option<LifecycleEffect> {
        self.waiting_command = None;
        self.protected_focus_token = None;
        self.provisional_focus = None;
        self.pending_focus = None;
        self.actions.clear();
        self.lifecycle.begin_unmount(now_ms).ok().flatten()
    }

    pub fn submit(
        &mut self,
        command: SourceCommand,
        current_document_id: Option<u64>,
        now_ms: u64,
    ) -> SubmitOutcome {
        self.submit_with_focus(command, current_document_id, now_ms, None)
    }

    pub fn submit_with_focus(
        &mut self,
        command: SourceCommand,
        current_document_id: Option<u64>,
        now_ms: u64,
        focus_token: Option<u64>,
    ) -> SubmitOutcome {
        if self.is_same_source_mode(&command) {
            return SubmitOutcome::NoOp;
        }
        match self.lifecycle.state.clone() {
            LifecycleState::Unmounted => {
                self.actions.push(ControllerAction::Execute {
                    command,
                    protected: false,
                    focus_token,
                });
                SubmitOutcome::ExecuteQueued
            }
            LifecycleState::Ready(editor)
                if current_document_id == Some(editor.identity.document_id) =>
            {
                match self.lifecycle.begin_snapshot(command, now_ms) {
                    Ok(effect @ LifecycleEffect::RequestSnapshot(_, operation_id)) => {
                        self.protected_focus_token = focus_token;
                        self.push_effect(effect);
                        SubmitOutcome::SnapshotRequested(operation_id)
                    }
                    _ => SubmitOutcome::Busy,
                }
            }
            LifecycleState::Mounting { ref intent, .. }
                if current_document_id == Some(intent.document_id) =>
            {
                self.queue_for_mount(command, focus_token)
            }
            LifecycleState::Initializing { identity, .. }
                if current_document_id == Some(identity.document_id) =>
            {
                self.queue_for_mount(command, focus_token)
            }
            LifecycleState::Unavailable { retired: None } => {
                self.actions.push(ControllerAction::Execute {
                    command,
                    protected: false,
                    focus_token,
                });
                SubmitOutcome::ExecuteQueued
            }
            LifecycleState::Unavailable { retired: Some(_) } => SubmitOutcome::Unavailable,
            _ => SubmitOutcome::Busy,
        }
    }

    pub fn handle_event(
        &mut self,
        event: SourceEditorEvent,
        app: &mut AppState,
        now_ms: u64,
    ) -> Result<EventOutcome, SourceSyncError> {
        if !event.has_supported_version() {
            return Err(SourceSyncError::UnsupportedVersion);
        }
        let result: Result<(), SourceSyncError> = (|| match event {
            event @ SourceEditorEvent::BundleReady { .. } => {
                self.lifecycle.handle_bundle_event(&event)?;
                if let Some(effect) = self.lifecycle.continue_mount_after_bundle(now_ms) {
                    self.push_effect(effect);
                }
                Ok(())
            }
            event @ SourceEditorEvent::BundleFailed { reason, .. } => {
                self.lifecycle.handle_bundle_event(&event)?;
                Err(reason.into())
            }
            event @ SourceEditorEvent::RelayReady { .. } => {
                if let Some(effect) = self.lifecycle.handle_relay_event(&event, now_ms)? {
                    self.push_effect(effect);
                }
                Ok(())
            }
            event @ SourceEditorEvent::RelayFailed { reason, .. } => {
                self.lifecycle.handle_relay_event(&event, now_ms)?;
                Err(reason.into())
            }
            event @ SourceEditorEvent::EditorReady { identity, .. } => {
                let ready_identity = identity;
                self.lifecycle.handle_init_event(&event)?;
                self.start_waiting_command(now_ms);
                self.queue_ready_focus(ready_identity);
                Ok(())
            }
            event @ SourceEditorEvent::InitFailed { reason, .. } => {
                self.lifecycle.handle_init_event(&event)?;
                self.start_waiting_command(now_ms);
                Err(reason.into())
            }
            event @ SourceEditorEvent::Change { .. } => self.accept_change(&event, app, now_ms),
            event @ SourceEditorEvent::Snapshot { .. } => self.accept_snapshot(&event, app, now_ms),
            event @ SourceEditorEvent::SnapshotBlocked { .. } => {
                let SourceEditorEvent::SnapshotBlocked { reason, .. } = &event else {
                    unreachable!()
                };
                let reason = *reason;
                self.lifecycle.handle_snapshot_blocked(&event)?;
                Err(reason.into())
            }
            event @ SourceEditorEvent::EditingResumed { .. } => {
                self.lifecycle.handle_resume_event(&event)?;
                Ok(())
            }
            event @ SourceEditorEvent::ResumeFailed { reason, .. } => {
                self.lifecycle.handle_resume_event(&event)?;
                Err(reason.into())
            }
            event @ SourceEditorEvent::DocumentApplied { .. } => {
                self.lifecycle.handle_document_event(&event)?;
                Ok(())
            }
            event @ SourceEditorEvent::ApplyDocumentFailed { reason, .. } => {
                self.lifecycle.handle_document_event(&event)?;
                Err(reason.into())
            }
            event @ SourceEditorEvent::Destroyed { .. } => {
                if let Some(effect) = self.lifecycle.handle_destroy_event(&event, now_ms)? {
                    self.push_effect(effect);
                }
                Ok(())
            }
            event @ SourceEditorEvent::DestroyFailed { reason, .. } => {
                self.lifecycle.handle_destroy_event(&event, now_ms)?;
                Err(reason.into())
            }
            SourceEditorEvent::Trace { .. } => Ok(()),
        })();
        if matches!(self.lifecycle.state, LifecycleState::Unavailable { .. }) {
            self.waiting_command = None;
            self.protected_focus_token = None;
        }
        match result {
            Ok(()) => Ok(EventOutcome::Applied),
            Err(SourceSyncError::Transition(
                TransitionError::Stale | TransitionError::InvalidState,
            )) => Ok(EventOutcome::Stale),
            Err(error) => Err(error),
        }
    }

    pub fn command_completed(
        &mut self,
        success: bool,
        after: SessionFingerprint,
        now_ms: u64,
    ) -> Result<CommandDisposition, SourceSyncError> {
        let (disposition, effect) = self.lifecycle.command_completed(success, after, now_ms)?;
        if let Some(effect) = effect {
            self.push_effect(effect);
        }
        Ok(disposition)
    }

    pub fn tick(&mut self, now_ms: u64) -> Result<TickOutcome, SourceSyncError> {
        if self.transport_is_holding_lifecycle_action() {
            return Ok(TickOutcome::Idle);
        }
        let Some(pending) = self.lifecycle.next_deadline() else {
            return Ok(TickOutcome::Idle);
        };
        if now_ms < pending.deadline_ms {
            return Ok(TickOutcome::Idle);
        }
        let takeover = matches!(
            self.lifecycle.state,
            LifecycleState::Unmounting {
                waiting: Some(_),
                ..
            }
        );
        if let Some(effect) = self.lifecycle.timeout(pending.operation_id, now_ms)? {
            self.push_effect(effect);
        }
        if matches!(self.lifecycle.state, LifecycleState::Unavailable { .. }) {
            self.waiting_command = None;
            self.protected_focus_token = None;
        }
        Ok(if takeover {
            TickOutcome::TakeoverStarted
        } else {
            TickOutcome::TimedOut
        })
    }

    pub fn is_ready(&self, editor_id: SourceEditorId, document_id: u64) -> bool {
        self.lifecycle.ready_editor().is_some_and(|editor| {
            editor.identity.editor_id == editor_id && editor.identity.document_id == document_id
        })
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self.lifecycle.state, LifecycleState::Unavailable { .. })
    }

    pub fn mount_handle(
        &self,
        editor_id: SourceEditorId,
        document_id: u64,
    ) -> Option<EditorMountHandle> {
        let identity = self.current_physical_identity()?;
        (identity.editor_id == editor_id && identity.document_id == document_id).then_some(
            EditorMountHandle {
                instance_id: identity.instance_id,
                editor_id,
                document_id,
            },
        )
    }

    pub fn drain_actions(&mut self) -> Vec<ControllerAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

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

    fn queue_for_mount(
        &mut self,
        command: SourceCommand,
        focus_token: Option<u64>,
    ) -> SubmitOutcome {
        if self.waiting_command.is_some() {
            SubmitOutcome::Busy
        } else {
            self.waiting_command = Some(PendingCommand {
                command,
                focus_token,
            });
            SubmitOutcome::WaitingForReady
        }
    }

    fn start_waiting_command(&mut self, now_ms: u64) {
        if !matches!(self.lifecycle.state, LifecycleState::Ready(_)) {
            self.waiting_command = None;
            return;
        }
        let Some(pending) = self.waiting_command.take() else {
            return;
        };
        if let Ok(effect) = self.lifecycle.begin_snapshot(pending.command, now_ms) {
            self.protected_focus_token = pending.focus_token;
            self.push_effect(effect);
        }
    }

    fn queue_ready_focus(&mut self, identity: EditorIdentity) {
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

    fn accept_change(
        &mut self,
        event: &SourceEditorEvent,
        app: &mut AppState,
        now_ms: u64,
    ) -> Result<(), SourceSyncError> {
        let SourceEditorEvent::Change {
            identity,
            seq,
            text,
            composing,
            ..
        } = event
        else {
            return Err(SourceSyncError::Transition(TransitionError::InvalidState));
        };
        let ready = self
            .lifecycle
            .ready_editor()
            .ok_or(SourceSyncError::EditorUnavailable)?;
        if *identity != ready.identity || *seq <= ready.last_seq || *composing {
            return Err(SourceSyncError::Transition(TransitionError::Stale));
        }
        let session = app.session.as_ref().ok_or(SourceSyncError::NoDocument)?;
        if session.document_id != identity.document_id || session.revision != ready.revision {
            return Err(SourceSyncError::RevisionDrift);
        }
        if session.canonical_text != *text {
            app.edit_text(ready.revision, text.clone(), now_ms)?;
        }
        let revision = app
            .session
            .as_ref()
            .ok_or(SourceSyncError::NoDocument)?
            .revision;
        self.lifecycle.accept_change(event, revision)?;
        Ok(())
    }

    fn accept_snapshot(
        &mut self,
        event: &SourceEditorEvent,
        app: &mut AppState,
        now_ms: u64,
    ) -> Result<(), SourceSyncError> {
        let LifecycleState::SnapshotPending {
            editor, operation, ..
        } = self.lifecycle.state.clone()
        else {
            return Err(SourceSyncError::Transition(TransitionError::InvalidState));
        };
        let SourceEditorEvent::Snapshot {
            operation_id,
            identity,
            seq,
            text,
            composing,
            ..
        } = event
        else {
            return Err(SourceSyncError::Transition(TransitionError::InvalidState));
        };
        if *operation_id != operation.operation_id || *identity != editor.identity {
            return Err(SourceSyncError::Transition(TransitionError::Stale));
        }
        let session = app.session.as_ref().ok_or(SourceSyncError::NoDocument)?;
        let stream_current = *seq >= editor.last_seq
            && !*composing
            && session.document_id == identity.document_id
            && session.revision == editor.revision;
        if !stream_current {
            if let Some(effect) = self.lifecycle.reject_snapshot(event, false, now_ms)? {
                self.push_effect(effect);
            }
            return Err(SourceSyncError::RevisionDrift);
        }
        if session.canonical_text != *text
            && let Err(error) = app.edit_text(editor.revision, text.clone(), now_ms)
        {
            if let Some(effect) = self.lifecycle.reject_snapshot(event, true, now_ms)? {
                self.push_effect(effect);
            }
            return Err(error.into());
        }
        let revision = app
            .session
            .as_ref()
            .ok_or(SourceSyncError::NoDocument)?
            .revision;
        let before = fingerprint(app);
        let effect = self.lifecycle.accept_snapshot(event, revision, before)?;
        self.push_effect(effect);
        Ok(())
    }

    fn push_effect(&mut self, effect: LifecycleEffect) {
        self.actions.push(match effect {
            LifecycleEffect::ExecuteCommand(command) => ControllerAction::Execute {
                command,
                protected: true,
                focus_token: self.protected_focus_token.take(),
            },
            effect => ControllerAction::Lifecycle(effect),
        });
    }
}

mod support;
pub use support::fingerprint;

#[cfg(test)]
mod tests;
