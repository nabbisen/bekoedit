//! Pure source-editor lifecycle reducer for RFC-041.

use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION,
    source_editor::{
        EditorIdentity, EditorInstanceId, OperationId, SourceEditorEvent, SourceEditorId,
        SourceEpoch, TakeoverPermit,
    },
};

use super::SourceCommand;

pub const MOUNT_DEADLINE_MS: u64 = 5_000;
pub const SNAPSHOT_DEADLINE_MS: u64 = 2_000;
pub const RESUME_DEADLINE_MS: u64 = 1_000;
pub const REFRESH_DEADLINE_MS: u64 = 2_000;
pub const DESTROY_DEADLINE_MS: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionFingerprint {
    pub document_id: Option<u64>,
    pub revision: Option<u64>,
    pub source_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountIntent {
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyEditor {
    pub identity: EditorIdentity,
    pub revision: u64,
    pub last_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldCertainty {
    Confirmed,
    Possible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldEditor {
    pub ready: ReadyEditor,
    pub snapshot_operation: OperationId,
    pub certainty: HoldCertainty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingOperation {
    pub operation_id: OperationId,
    pub deadline_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LifecycleState {
    #[default]
    Unmounted,
    Mounting {
        intent: MountIntent,
        identity: EditorIdentity,
        relay: PendingOperation,
        relay_ready: bool,
        bundle_ready: bool,
        takeover: Option<TakeoverPermit>,
    },
    Initializing {
        identity: EditorIdentity,
        revision: u64,
        operation: PendingOperation,
    },
    Ready(ReadyEditor),
    SnapshotPending {
        editor: ReadyEditor,
        command: SourceCommand,
        operation: PendingOperation,
    },
    BarrierHeld {
        editor: HeldEditor,
        command: SourceCommand,
        before: SessionFingerprint,
    },
    ResumePending {
        editor: HeldEditor,
        operation: PendingOperation,
    },
    RefreshPending {
        editor: HeldEditor,
        new_epoch: SourceEpoch,
        revision: u64,
        operation: PendingOperation,
    },
    Unmounting {
        retired: EditorIdentity,
        operation: PendingOperation,
        waiting: Option<MountIntent>,
    },
    Unavailable {
        retired: Option<EditorIdentity>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEffect {
    InstallRelay(EditorIdentity, OperationId),
    Init(EditorIdentity, OperationId, Option<TakeoverPermit>),
    RequestSnapshot(EditorIdentity, OperationId),
    ExecuteCommand(SourceCommand),
    Resume(EditorIdentity, OperationId, OperationId),
    ApplyDocument(EditorIdentity, SourceEpoch, OperationId),
    Destroy(EditorIdentity, OperationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    Busy,
    Stale,
    UnsupportedVersion,
    InvalidState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandDisposition {
    Resume,
    Refresh { revision: u64 },
    Destroy,
    Unavailable,
}

#[derive(Debug, Default)]
pub struct LifecycleReducer {
    pub state: LifecycleState,
    bundle_ready: bool,
    bundle_probe: Option<PendingOperation>,
    next_instance: u64,
    next_epoch: u64,
    next_operation: u64,
    next_nonce: u64,
}

impl LifecycleReducer {
    pub fn begin_bundle_probe(&mut self, now_ms: u64) -> OperationId {
        let operation = self.operation(now_ms, MOUNT_DEADLINE_MS);
        self.bundle_probe = Some(operation);
        operation.operation_id
    }

    pub fn handle_bundle_event(
        &mut self,
        event: &SourceEditorEvent,
    ) -> Result<(), TransitionError> {
        self.require_version(event)?;
        match event {
            SourceEditorEvent::BundleReady { operation_id, .. } => {
                let pending = self.bundle_probe.ok_or(TransitionError::Stale)?;
                if pending.operation_id != *operation_id {
                    return Err(TransitionError::Stale);
                }
                self.bundle_probe = None;
                self.bundle_ready = true;
                if let LifecycleState::Mounting { bundle_ready, .. } = &mut self.state {
                    *bundle_ready = true;
                }
                Ok(())
            }
            SourceEditorEvent::BundleFailed { operation_id, .. } => {
                self.match_bundle(*operation_id)?;
                self.bundle_probe = None;
                self.state = LifecycleState::Unavailable { retired: None };
                Ok(())
            }
            _ => Err(TransitionError::InvalidState),
        }
    }

    pub fn begin_mount(
        &mut self,
        intent: MountIntent,
        now_ms: u64,
    ) -> Result<LifecycleEffect, TransitionError> {
        match self.state.clone() {
            LifecycleState::Unmounted => self.start_mount(intent, now_ms, None),
            LifecycleState::Unavailable { retired: None } => self.start_mount(intent, now_ms, None),
            LifecycleState::Unavailable {
                retired: Some(retired),
            } => self.start_destroy(retired, Some(intent), now_ms),
            LifecycleState::Ready(editor) => {
                self.start_destroy(editor.identity, Some(intent), now_ms)
            }
            LifecycleState::BarrierHeld { editor, .. }
            | LifecycleState::ResumePending { editor, .. }
            | LifecycleState::RefreshPending { editor, .. } => {
                self.start_destroy(editor.ready.identity, Some(intent), now_ms)
            }
            LifecycleState::Unmounting {
                retired,
                operation,
                waiting: None,
            } => {
                self.state = LifecycleState::Unmounting {
                    retired,
                    operation,
                    waiting: Some(intent),
                };
                Err(TransitionError::Busy)
            }
            _ => Err(TransitionError::Busy),
        }
    }

    pub fn handle_relay_event(
        &mut self,
        event: &SourceEditorEvent,
    ) -> Result<Option<LifecycleEffect>, TransitionError> {
        self.require_version(event)?;
        let LifecycleState::Mounting {
            identity,
            intent,
            relay,
            relay_ready,
            bundle_ready,
            takeover,
        } = &mut self.state
        else {
            return Err(TransitionError::InvalidState);
        };
        match event {
            SourceEditorEvent::RelayReady {
                operation_id,
                identity: actual,
                ..
            } if *operation_id == relay.operation_id && actual == identity => {
                *relay_ready = true;
                if *bundle_ready {
                    let identity = *identity;
                    let revision = intent.revision;
                    let takeover = takeover.clone();
                    let operation = *relay;
                    self.state = LifecycleState::Initializing {
                        identity,
                        revision,
                        operation,
                    };
                    Ok(Some(LifecycleEffect::Init(
                        identity,
                        operation.operation_id,
                        takeover,
                    )))
                } else {
                    Ok(None)
                }
            }
            SourceEditorEvent::RelayFailed {
                operation_id,
                identity: actual,
                ..
            } if *operation_id == relay.operation_id && actual == identity => {
                let retired = Some(*identity);
                self.state = LifecycleState::Unavailable { retired };
                Ok(None)
            }
            _ => Err(TransitionError::Stale),
        }
    }

    pub fn continue_mount_after_bundle(&mut self) -> Option<LifecycleEffect> {
        let LifecycleState::Mounting {
            identity,
            intent,
            relay,
            relay_ready: true,
            bundle_ready: true,
            takeover,
        } = &self.state
        else {
            return None;
        };
        let effect = LifecycleEffect::Init(*identity, relay.operation_id, takeover.clone());
        self.state = LifecycleState::Initializing {
            identity: *identity,
            revision: intent.revision,
            operation: *relay,
        };
        Some(effect)
    }

    pub fn handle_init_event(&mut self, event: &SourceEditorEvent) -> Result<(), TransitionError> {
        self.require_version(event)?;
        let LifecycleState::Initializing {
            identity,
            revision,
            operation,
        } = self.state
        else {
            return Err(TransitionError::InvalidState);
        };
        match event {
            SourceEditorEvent::EditorReady {
                operation_id,
                identity: actual,
                revision: actual_revision,
                ..
            } if *operation_id == operation.operation_id
                && *actual == identity
                && *actual_revision == revision =>
            {
                self.state = LifecycleState::Ready(ReadyEditor {
                    identity,
                    revision,
                    last_seq: 0,
                });
                Ok(())
            }
            SourceEditorEvent::InitFailed {
                operation_id,
                identity: actual,
                ..
            } if *operation_id == operation.operation_id && *actual == identity => {
                self.state = LifecycleState::Unavailable {
                    retired: Some(identity),
                };
                Ok(())
            }
            _ => Err(TransitionError::Stale),
        }
    }

    pub fn handle_destroy_event(
        &mut self,
        event: &SourceEditorEvent,
        now_ms: u64,
    ) -> Result<Option<LifecycleEffect>, TransitionError> {
        self.require_version(event)?;
        let LifecycleState::Unmounting {
            retired,
            operation,
            waiting,
        } = self.state.clone()
        else {
            return Err(TransitionError::InvalidState);
        };
        match event {
            SourceEditorEvent::Destroyed {
                operation_id,
                identity,
                ..
            } if *operation_id == operation.operation_id && *identity == retired => {
                self.state = LifecycleState::Unmounted;
                waiting
                    .map(|intent| self.start_mount(intent, now_ms, None))
                    .transpose()
            }
            SourceEditorEvent::DestroyFailed {
                operation_id,
                identity,
                ..
            } if *operation_id == operation.operation_id && *identity == retired => {
                self.state = LifecycleState::Unavailable {
                    retired: Some(retired),
                };
                Ok(None)
            }
            _ => Err(TransitionError::Stale),
        }
    }

    fn start_mount(
        &mut self,
        intent: MountIntent,
        now_ms: u64,
        takeover: Option<TakeoverPermit>,
    ) -> Result<LifecycleEffect, TransitionError> {
        let identity = self.allocate_identity(&intent)?;
        Ok(self.start_mount_with_identity(intent, identity, now_ms, takeover))
    }

    fn start_mount_with_identity(
        &mut self,
        intent: MountIntent,
        identity: EditorIdentity,
        now_ms: u64,
        takeover: Option<TakeoverPermit>,
    ) -> LifecycleEffect {
        let relay = self.operation(now_ms, MOUNT_DEADLINE_MS);
        self.state = LifecycleState::Mounting {
            intent,
            identity,
            relay,
            relay_ready: false,
            bundle_ready: self.bundle_ready,
            takeover,
        };
        LifecycleEffect::InstallRelay(identity, relay.operation_id)
    }

    fn start_resume(&mut self, editor: HeldEditor, now_ms: u64) -> LifecycleEffect {
        let operation = self.operation(now_ms, RESUME_DEADLINE_MS);
        self.state = LifecycleState::ResumePending { editor, operation };
        LifecycleEffect::Resume(
            editor.ready.identity,
            editor.snapshot_operation,
            operation.operation_id,
        )
    }

    fn start_refresh(&mut self, editor: HeldEditor, revision: u64, now_ms: u64) -> LifecycleEffect {
        let new_epoch = self.allocate_epoch().unwrap_or(editor.ready.identity.epoch);
        let operation = self.operation(now_ms, REFRESH_DEADLINE_MS);
        self.state = LifecycleState::RefreshPending {
            editor,
            new_epoch,
            revision,
            operation,
        };
        LifecycleEffect::ApplyDocument(editor.ready.identity, new_epoch, operation.operation_id)
    }

    fn start_destroy(
        &mut self,
        retired: EditorIdentity,
        waiting: Option<MountIntent>,
        now_ms: u64,
    ) -> Result<LifecycleEffect, TransitionError> {
        let operation = self.operation(now_ms, DESTROY_DEADLINE_MS);
        self.state = LifecycleState::Unmounting {
            retired,
            operation,
            waiting,
        };
        Ok(LifecycleEffect::Destroy(retired, operation.operation_id))
    }

    fn allocate_identity(
        &mut self,
        intent: &MountIntent,
    ) -> Result<EditorIdentity, TransitionError> {
        self.next_instance = self
            .next_instance
            .checked_add(1)
            .ok_or(TransitionError::InvalidState)?;
        Ok(EditorIdentity {
            instance_id: EditorInstanceId::new(self.next_instance),
            editor_id: intent.editor_id,
            document_id: intent.document_id,
            epoch: self.allocate_epoch()?,
        })
    }

    fn allocate_epoch(&mut self) -> Result<SourceEpoch, TransitionError> {
        self.next_epoch = self
            .next_epoch
            .checked_add(1)
            .ok_or(TransitionError::InvalidState)?;
        Ok(SourceEpoch::new(self.next_epoch))
    }

    fn operation(&mut self, now_ms: u64, timeout: u64) -> PendingOperation {
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .expect("operation id exhausted");
        PendingOperation {
            operation_id: OperationId::new(self.next_operation),
            deadline_ms: now_ms.saturating_add(timeout),
        }
    }

    fn require_version(&self, event: &SourceEditorEvent) -> Result<(), TransitionError> {
        if event.protocol_version() == BRIDGE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(TransitionError::UnsupportedVersion)
        }
    }

    fn match_bundle(&self, operation_id: OperationId) -> Result<(), TransitionError> {
        if self
            .bundle_probe
            .is_some_and(|p| p.operation_id == operation_id)
        {
            Ok(())
        } else {
            Err(TransitionError::Stale)
        }
    }

    fn expired(pending: PendingOperation, operation_id: OperationId, now_ms: u64) -> bool {
        pending.operation_id == operation_id && now_ms >= pending.deadline_ms
    }
}

mod transitions;

#[cfg(test)]
mod tests;
