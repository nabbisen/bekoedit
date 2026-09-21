//! RFC-047 slice 1: the queue that holds a user command behind a lifecycle
//! transition, where `submit` used to answer `Busy` and the caller dropped it.
//!
//! One queue, no second mechanism: it replaces the depth-1 slot that held a
//! command while the editor was mounting. Every way out of it is one of
//! executed, superseded by the user's own newer command, or recorded in
//! `discards` for the host to report.

use std::collections::VecDeque;

use super::super::SourceCommand;
use super::super::lifecycle::{LifecycleEffect, LifecycleState};
use super::types::{
    DiscardReason, QUEUE_DEPTH, QUEUE_ENTRY_DEADLINE_MS, QueueDiscard, QueueScope, QueuedCommand,
};
use super::{ControllerAction, SourceSyncState, SubmitOutcome};

/// What `submit` does with a command in the current lifecycle state.
enum Gate {
    /// Act on it now.
    Open,
    /// A transition is under way: hold it in the queue.
    Held,
    /// Waiting cannot help (RFC-047 §5.6): report it now.
    Unavailable,
}

/// Exhaustive on purpose: a new command must be classified before it compiles.
fn scope_of(command: &SourceCommand, current_document_id: Option<u64>) -> QueueScope {
    match command {
        SourceCommand::SaveNow
        | SourceCommand::SaveAs(_)
        | SourceCommand::MoveSectionUp(_)
        | SourceCommand::MoveSectionDown(_)
        | SourceCommand::RestoreHistory(_) => QueueScope::Document(current_document_id),
        SourceCommand::SwitchMode(_)
        | SourceCommand::OpenSettings
        | SourceCommand::OpenDocument(_)
        | SourceCommand::NewUntitled
        | SourceCommand::OpenWorkspace(_)
        | SourceCommand::CloseWorkspace => QueueScope::Anywhere,
    }
}

impl SourceSyncState {
    /// Mirrors what `submit` did before the queue, arm for arm. Exhaustive, so
    /// a new `LifecycleState` must be classified here.
    fn gate(&self, current_document_id: Option<u64>) -> Gate {
        match &self.lifecycle.state {
            LifecycleState::Unmounted | LifecycleState::Unavailable { retired: None } => Gate::Open,
            LifecycleState::Ready(editor) => {
                if current_document_id == Some(editor.identity.document_id) {
                    Gate::Open
                } else {
                    Gate::Held
                }
            }
            LifecycleState::Mounting { .. }
            | LifecycleState::Initializing { .. }
            | LifecycleState::SnapshotPending { .. }
            | LifecycleState::BarrierHeld { .. }
            | LifecycleState::ResumePending { .. }
            | LifecycleState::RefreshPending { .. }
            | LifecycleState::Unmounting { .. } => Gate::Held,
            LifecycleState::Unavailable { retired: Some(_) } => Gate::Unavailable,
        }
    }

    /// The submit path: run the command if the controller would accept it and
    /// nothing is waiting ahead of it, otherwise queue it.
    pub(super) fn submit_or_queue(
        &mut self,
        command: SourceCommand,
        current_document_id: Option<u64>,
        now_ms: u64,
        focus_token: Option<u64>,
    ) -> SubmitOutcome {
        self.drain_queue(current_document_id, now_ms);
        match self.gate(current_document_id) {
            Gate::Unavailable => SubmitOutcome::Unavailable,
            Gate::Open if self.queue.is_empty() => {
                match self.run_now(command, now_ms, focus_token) {
                    Ok(outcome) => outcome,
                    Err(command) => self.enqueue(command, current_document_id, now_ms, focus_token),
                }
            }
            Gate::Open | Gate::Held => {
                self.enqueue(command, current_document_id, now_ms, focus_token)
            }
        }
    }

    /// Acts on a command in a state whose gate is `Open`. Hands the command
    /// back if the state refuses it after all.
    fn run_now(
        &mut self,
        command: SourceCommand,
        now_ms: u64,
        focus_token: Option<u64>,
    ) -> Result<SubmitOutcome, SourceCommand> {
        match &self.lifecycle.state {
            LifecycleState::Unmounted | LifecycleState::Unavailable { retired: None } => {
                self.actions.push(ControllerAction::Execute {
                    command,
                    protected: false,
                    focus_token,
                });
                Ok(SubmitOutcome::ExecuteQueued)
            }
            LifecycleState::Ready(_) => {
                match self.lifecycle.begin_snapshot(command.clone(), now_ms) {
                    Ok(effect @ LifecycleEffect::RequestSnapshot(_, operation_id)) => {
                        self.protected_focus_token = focus_token;
                        self.push_effect(effect);
                        Ok(SubmitOutcome::SnapshotRequested(operation_id))
                    }
                    _ => Err(command),
                }
            }
            _ => Err(command),
        }
    }

    fn enqueue(
        &mut self,
        command: SourceCommand,
        current_document_id: Option<u64>,
        now_ms: u64,
        focus_token: Option<u64>,
    ) -> SubmitOutcome {
        let accepted = if matches!(
            self.lifecycle.state,
            LifecycleState::Mounting { .. } | LifecycleState::Initializing { .. }
        ) {
            SubmitOutcome::WaitingForReady
        } else {
            SubmitOutcome::Queued
        };
        let entry = QueuedCommand {
            scope: scope_of(&command, current_document_id),
            deadline_ms: now_ms.saturating_add(QUEUE_ENTRY_DEADLINE_MS),
            command,
            focus_token,
        };
        // Coalescing (RFC-047 §5.2). It takes no capacity.
        match &entry.command {
            // The user changed their mind: the newer mode replaces the queued
            // one, so no intermediate mode is shown.
            SourceCommand::SwitchMode(_) => {
                if let Some(slot) = self
                    .queue
                    .iter_mut()
                    .find(|queued| matches!(queued.command, SourceCommand::SwitchMode(_)))
                {
                    *slot = entry;
                    return accepted;
                }
            }
            // Idempotent, so the queued save stands for both -- but only for
            // the same document. A save for another one is its own intent.
            SourceCommand::SaveNow => {
                if self
                    .queue
                    .iter()
                    .any(|queued| queued.command == entry.command && queued.scope == entry.scope)
                {
                    return accepted;
                }
            }
            _ => {}
        }
        if self.queue.len() >= QUEUE_DEPTH {
            // The newest is refused. Evicting the oldest would lose a command
            // the user asked for earlier, silently.
            self.record_discard(entry, DiscardReason::Overflow);
            return SubmitOutcome::QueueFull;
        }
        self.queue.push_back(entry);
        accepted
    }

    /// Runs whatever is at the front of the queue that the controller would
    /// accept now, and settles everything that cannot run. Called after every
    /// handled event, on `tick`, and by `submit`: nothing else drives it.
    pub(super) fn drain_queue(&mut self, current_document_id: Option<u64>, now_ms: u64) {
        self.expire_queue(now_ms);
        while let Some(front) = self.queue.front().cloned() {
            match self.gate(current_document_id) {
                Gate::Held => break,
                Gate::Unavailable => {
                    self.discard_queue(DiscardReason::EditorUnavailable);
                    break;
                }
                Gate::Open => {}
            }
            if let QueueScope::Document(recorded) = front.scope {
                // An unprotected command is executed by the host, not here.
                // Until it has been, the open document may still change, so a
                // document-scoped entry behind it cannot be judged yet
                // (RFC-047 §5.4). Entries that name no document run in order.
                if self.has_pending_execute() {
                    break;
                }
                if recorded != current_document_id {
                    self.queue.pop_front();
                    self.record_discard(front, DiscardReason::DocumentChanged);
                    continue;
                }
            }
            if self.is_same_source_mode(&front.command) {
                // Became a no-op while it waited: nothing to run, nothing to say.
                self.queue.pop_front();
                continue;
            }
            match self.run_now(front.command, now_ms, front.focus_token) {
                Ok(_) => {
                    self.queue.pop_front();
                }
                Err(_) => break,
            }
        }
    }

    fn has_pending_execute(&self) -> bool {
        self.actions
            .iter()
            .any(|action| matches!(action, ControllerAction::Execute { .. }))
    }

    fn expire_queue(&mut self, now_ms: u64) {
        let (expired, kept): (VecDeque<_>, VecDeque<_>) = self
            .queue
            .drain(..)
            .partition(|entry| now_ms >= entry.deadline_ms);
        self.queue = kept;
        for entry in expired {
            self.record_discard(entry, DiscardReason::Expired);
        }
    }

    /// Empties the queue, recording every entry with `reason`.
    pub(super) fn discard_queue(&mut self, reason: DiscardReason) {
        for entry in std::mem::take(&mut self.queue) {
            self.record_discard(entry, reason);
        }
    }

    fn record_discard(&mut self, entry: QueuedCommand, reason: DiscardReason) {
        self.discards.push(QueueDiscard {
            command: entry.command,
            reason,
            focus_token: entry.focus_token,
        });
    }

    /// Commands that left the queue without running, oldest first. The host
    /// reports them; the controller never forgets one silently.
    pub fn drain_discards(&mut self) -> Vec<QueueDiscard> {
        std::mem::take(&mut self.discards)
    }

    pub fn has_discards(&self) -> bool {
        !self.discards.is_empty()
    }
}
