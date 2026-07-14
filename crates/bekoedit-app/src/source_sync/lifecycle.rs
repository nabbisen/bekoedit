//! Pure source-editor lifecycle reducer for RFC-041.

use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEditorId, SourceEpoch,
};

pub const MOUNT_DEADLINE_MS: u64 = 5_000;
pub const SNAPSHOT_DEADLINE_MS: u64 = 2_000;
pub const RESUME_DEADLINE_MS: u64 = 1_000;
pub const REFRESH_DEADLINE_MS: u64 = 2_000;
pub const DESTROY_DEADLINE_MS: u64 = 1_000;

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
pub struct PendingOperation<T> {
    pub operation_id: OperationId,
    pub deadline_ms: u64,
    pub value: T,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LifecycleState {
    #[default]
    Unmounted,
    Mounting {
        identity: EditorIdentity,
        revision: u64,
        relay_ready: bool,
        bundle_ready: bool,
        operation: PendingOperation<()>,
    },
    Initializing {
        identity: EditorIdentity,
        revision: u64,
        operation: PendingOperation<()>,
    },
    Ready(ReadyEditor),
    SnapshotPending {
        editor: ReadyEditor,
        operation: PendingOperation<()>,
    },
    BarrierHeld(HeldEditor),
    ResumePending {
        editor: HeldEditor,
        operation: PendingOperation<()>,
    },
    RefreshPending {
        editor: HeldEditor,
        new_epoch: SourceEpoch,
        revision: u64,
        operation: PendingOperation<()>,
    },
    Unmounting {
        identity: EditorIdentity,
        operation: PendingOperation<()>,
    },
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEffect {
    InstallRelay(EditorIdentity),
    Init(EditorIdentity, OperationId),
    RequestSnapshot(EditorIdentity, OperationId),
    Resume(EditorIdentity, OperationId, OperationId),
    ApplyDocument(EditorIdentity, SourceEpoch, OperationId),
    Destroy(EditorIdentity, OperationId),
}

#[derive(Debug, Default)]
pub struct LifecycleReducer {
    pub state: LifecycleState,
    next_instance: u64,
    next_epoch: u64,
    next_operation: u64,
}

impl LifecycleReducer {
    pub fn begin_mount(
        &mut self,
        editor_id: SourceEditorId,
        document_id: u64,
        revision: u64,
        now_ms: u64,
    ) -> LifecycleEffect {
        let identity = EditorIdentity {
            instance_id: self.allocate_instance(),
            editor_id,
            document_id,
            epoch: self.allocate_epoch(),
        };
        let operation = self.operation(now_ms, MOUNT_DEADLINE_MS);
        self.state = LifecycleState::Mounting {
            identity,
            revision,
            relay_ready: false,
            bundle_ready: false,
            operation,
        };
        LifecycleEffect::InstallRelay(identity)
    }

    pub fn mark_bundle_ready(&mut self) -> Option<LifecycleEffect> {
        self.mark_mount_prerequisite(true)
    }

    pub fn mark_relay_ready(&mut self, instance: EditorInstanceId) -> Option<LifecycleEffect> {
        let LifecycleState::Mounting { identity, .. } = self.state else {
            return None;
        };
        if identity.instance_id != instance {
            return None;
        }
        self.mark_mount_prerequisite(false)
    }

    pub fn editor_ready(&mut self, identity: EditorIdentity, operation_id: OperationId) -> bool {
        let LifecycleState::Initializing {
            identity: expected,
            revision,
            operation,
        } = self.state
        else {
            return false;
        };
        if expected != identity || operation.operation_id != operation_id {
            return false;
        }
        self.state = LifecycleState::Ready(ReadyEditor {
            identity,
            revision,
            last_seq: 0,
        });
        true
    }

    pub fn begin_snapshot(&mut self, now_ms: u64) -> Option<LifecycleEffect> {
        let LifecycleState::Ready(editor) = self.state else {
            return None;
        };
        let operation = self.operation(now_ms, SNAPSHOT_DEADLINE_MS);
        self.state = LifecycleState::SnapshotPending { editor, operation };
        Some(LifecycleEffect::RequestSnapshot(
            editor.identity,
            operation.operation_id,
        ))
    }

    pub fn snapshot_received(&mut self, operation_id: OperationId, seq: u64) -> bool {
        let LifecycleState::SnapshotPending {
            mut editor,
            operation,
        } = self.state
        else {
            return false;
        };
        if operation.operation_id != operation_id {
            return false;
        }
        editor.last_seq = seq;
        self.state = LifecycleState::BarrierHeld(HeldEditor {
            ready: editor,
            snapshot_operation: operation_id,
            certainty: HoldCertainty::Confirmed,
        });
        true
    }

    pub fn begin_resume(&mut self, now_ms: u64) -> Option<LifecycleEffect> {
        let editor = match self.state {
            LifecycleState::BarrierHeld(editor) => editor,
            LifecycleState::SnapshotPending { editor, operation } => HeldEditor {
                ready: editor,
                snapshot_operation: operation.operation_id,
                certainty: HoldCertainty::Possible,
            },
            _ => return None,
        };
        let operation = self.operation(now_ms, RESUME_DEADLINE_MS);
        self.state = LifecycleState::ResumePending { editor, operation };
        Some(LifecycleEffect::Resume(
            editor.ready.identity,
            editor.snapshot_operation,
            operation.operation_id,
        ))
    }

    pub fn editing_resumed(
        &mut self,
        operation_id: OperationId,
        snapshot_operation: OperationId,
    ) -> bool {
        let LifecycleState::ResumePending { editor, operation } = self.state else {
            return false;
        };
        if operation.operation_id != operation_id || editor.snapshot_operation != snapshot_operation
        {
            return false;
        }
        self.state = LifecycleState::Ready(editor.ready);
        true
    }

    pub fn begin_refresh(&mut self, revision: u64, now_ms: u64) -> Option<LifecycleEffect> {
        let LifecycleState::BarrierHeld(editor) = self.state else {
            return None;
        };
        let new_epoch = self.allocate_epoch();
        let operation = self.operation(now_ms, REFRESH_DEADLINE_MS);
        self.state = LifecycleState::RefreshPending {
            editor,
            new_epoch,
            revision,
            operation,
        };
        Some(LifecycleEffect::ApplyDocument(
            editor.ready.identity,
            new_epoch,
            operation.operation_id,
        ))
    }

    pub fn document_applied(
        &mut self,
        operation_id: OperationId,
        epoch: SourceEpoch,
        revision: u64,
    ) -> bool {
        let LifecycleState::RefreshPending {
            editor,
            new_epoch,
            revision: expected_revision,
            operation,
        } = self.state
        else {
            return false;
        };
        if operation.operation_id != operation_id
            || new_epoch != epoch
            || expected_revision != revision
        {
            return false;
        }
        let mut identity = editor.ready.identity;
        identity.epoch = new_epoch;
        self.state = LifecycleState::Ready(ReadyEditor {
            identity,
            revision,
            last_seq: 0,
        });
        true
    }

    pub fn begin_destroy(&mut self, now_ms: u64) -> Option<LifecycleEffect> {
        let identity = match self.state {
            LifecycleState::Ready(editor) => editor.identity,
            LifecycleState::BarrierHeld(editor) => editor.ready.identity,
            LifecycleState::ResumePending { editor, .. } => editor.ready.identity,
            LifecycleState::RefreshPending { editor, .. } => editor.ready.identity,
            LifecycleState::Unavailable => return None,
            _ => return None,
        };
        let operation = self.operation(now_ms, DESTROY_DEADLINE_MS);
        self.state = LifecycleState::Unmounting {
            identity,
            operation,
        };
        Some(LifecycleEffect::Destroy(identity, operation.operation_id))
    }

    pub fn destroyed(&mut self, instance: EditorInstanceId, operation_id: OperationId) -> bool {
        let LifecycleState::Unmounting {
            identity,
            operation,
        } = self.state
        else {
            return false;
        };
        if identity.instance_id != instance || operation.operation_id != operation_id {
            return false;
        }
        self.state = LifecycleState::Unmounted;
        true
    }

    pub fn expire(&mut self, operation_id: OperationId, now_ms: u64) -> bool {
        let pending = match self.state {
            LifecycleState::Mounting { operation, .. }
            | LifecycleState::Initializing { operation, .. }
            | LifecycleState::SnapshotPending { operation, .. }
            | LifecycleState::ResumePending { operation, .. }
            | LifecycleState::RefreshPending { operation, .. }
            | LifecycleState::Unmounting { operation, .. } => operation,
            _ => return false,
        };
        if pending.operation_id != operation_id || now_ms < pending.deadline_ms {
            return false;
        }
        self.state = LifecycleState::Unavailable;
        true
    }

    fn mark_mount_prerequisite(&mut self, bundle: bool) -> Option<LifecycleEffect> {
        let LifecycleState::Mounting {
            identity,
            revision,
            mut relay_ready,
            mut bundle_ready,
            operation,
        } = self.state
        else {
            return None;
        };
        if bundle {
            bundle_ready = true;
        } else {
            relay_ready = true;
        }
        if relay_ready && bundle_ready {
            self.state = LifecycleState::Initializing {
                identity,
                revision,
                operation,
            };
            Some(LifecycleEffect::Init(identity, operation.operation_id))
        } else {
            self.state = LifecycleState::Mounting {
                identity,
                revision,
                relay_ready,
                bundle_ready,
                operation,
            };
            None
        }
    }

    fn allocate_instance(&mut self) -> EditorInstanceId {
        self.next_instance += 1;
        EditorInstanceId(self.next_instance)
    }

    fn allocate_epoch(&mut self) -> SourceEpoch {
        self.next_epoch += 1;
        SourceEpoch(self.next_epoch)
    }

    fn operation(&mut self, now_ms: u64, timeout_ms: u64) -> PendingOperation<()> {
        self.next_operation += 1;
        PendingOperation {
            operation_id: OperationId(self.next_operation),
            deadline_ms: now_ms.saturating_add(timeout_ms),
            value: (),
        }
    }
}

#[cfg(test)]
mod tests;
