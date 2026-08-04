use super::lifecycle::{
    CommandDisposition, LifecycleEffect, LifecycleState, MountIntent, SessionFingerprint,
    TransitionError,
};
use super::{SourceCommand, SourceSyncError};

use types::PendingCommand;
pub use types::{
    ControllerAction, EditorMountHandle, EventOutcome, FocusClaim, FocusResolution, MountOutcome,
    SourceSyncState, SubmitOutcome, TickOutcome,
};

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
        self.shell_focus_held = false;
        self.actions.clear();
        self.lifecycle.begin_unmount(now_ms).ok().flatten()
    }

    /// Shell surfaces (menus, transient panels, screen replacements) call this
    /// before moving DOM focus. It cancels any pending source-focus intent —
    /// same as `cancel_focus_interactions` — and additionally claims shell
    /// authority so no *new* focus intent can be recorded until released
    /// (RFC-042 §6). Returns the cancelled interaction's guard token, if any.
    pub fn acquire_shell_focus(&mut self) -> Option<u64> {
        self.shell_focus_held = true;
        self.cancel_focus_interactions()
    }

    /// Releases shell authority. Source-focus intents may be recorded again.
    pub fn release_shell_focus(&mut self) {
        self.shell_focus_held = false;
    }

    /// True while a shell surface holds focus authority.
    pub fn shell_focus_held(&self) -> bool {
        self.shell_focus_held
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

mod interaction;

mod support;
pub use support::fingerprint;

mod events;
mod types;

#[cfg(test)]
mod tests;
