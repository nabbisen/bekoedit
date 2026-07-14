use super::*;

impl LifecycleReducer {
    pub fn abandon_bundle_probe(&mut self) -> bool {
        let abandoned = self.bundle_probe.take().is_some();
        if abandoned {
            self.bundle_ready = false;
        }
        abandoned
    }

    pub fn rearm_transport_deadlines(&mut self, now_ms: u64) {
        if let Some(operation) = &mut self.bundle_probe {
            operation.deadline_ms = now_ms.saturating_add(MOUNT_DEADLINE_MS);
        }
        let pending = match &mut self.state {
            LifecycleState::Mounting { relay, .. }
            | LifecycleState::Initializing {
                operation: relay, ..
            } => Some((relay, MOUNT_DEADLINE_MS)),
            LifecycleState::SnapshotPending { operation, .. } => {
                Some((operation, SNAPSHOT_DEADLINE_MS))
            }
            LifecycleState::ResumePending { operation, .. } => {
                Some((operation, RESUME_DEADLINE_MS))
            }
            LifecycleState::RefreshPending { operation, .. } => {
                Some((operation, REFRESH_DEADLINE_MS))
            }
            LifecycleState::Unmounting { operation, .. } => Some((operation, DESTROY_DEADLINE_MS)),
            _ => None,
        };
        if let Some((operation, duration_ms)) = pending {
            operation.deadline_ms = now_ms.saturating_add(duration_ms);
        }
    }

    pub fn ready_editor(&self) -> Option<ReadyEditor> {
        match self.state {
            LifecycleState::Ready(editor) => Some(editor),
            _ => None,
        }
    }

    pub fn next_deadline(&self) -> Option<PendingOperation> {
        let state_pending = match self.state {
            LifecycleState::Mounting { relay, .. } => Some(relay),
            LifecycleState::Initializing { operation, .. }
            | LifecycleState::SnapshotPending { operation, .. }
            | LifecycleState::ResumePending { operation, .. }
            | LifecycleState::RefreshPending { operation, .. }
            | LifecycleState::Unmounting { operation, .. } => Some(operation),
            _ => None,
        };
        match (self.bundle_probe, state_pending) {
            (Some(bundle), Some(state)) if bundle.deadline_ms <= state.deadline_ms => Some(bundle),
            (Some(_), Some(state)) => Some(state),
            (Some(bundle), None) => Some(bundle),
            (None, state) => state,
        }
    }

    pub fn begin_unmount(
        &mut self,
        now_ms: u64,
    ) -> Result<Option<LifecycleEffect>, TransitionError> {
        let retired = match self.state.clone() {
            LifecycleState::Unmounted | LifecycleState::Unmounting { .. } => return Ok(None),
            LifecycleState::Unavailable { retired: None } => {
                self.state = LifecycleState::Unmounted;
                return Ok(None);
            }
            LifecycleState::Mounting { identity, .. }
            | LifecycleState::Initializing { identity, .. }
            | LifecycleState::Ready(ReadyEditor { identity, .. })
            | LifecycleState::SnapshotPending {
                editor: ReadyEditor { identity, .. },
                ..
            } => identity,
            LifecycleState::BarrierHeld { editor, .. }
            | LifecycleState::ResumePending { editor, .. }
            | LifecycleState::RefreshPending { editor, .. } => editor.ready.identity,
            LifecycleState::Unavailable {
                retired: Some(identity),
            } => identity,
        };
        self.start_destroy(retired, None, now_ms).map(Some)
    }

    pub fn relay_lost(&mut self) -> bool {
        let retired = match self.state.clone() {
            LifecycleState::Unmounted | LifecycleState::Unavailable { .. } => return false,
            LifecycleState::Mounting { identity, .. }
            | LifecycleState::Initializing { identity, .. }
            | LifecycleState::Ready(ReadyEditor { identity, .. })
            | LifecycleState::SnapshotPending {
                editor: ReadyEditor { identity, .. },
                ..
            } => identity,
            LifecycleState::BarrierHeld { editor, .. }
            | LifecycleState::ResumePending { editor, .. }
            | LifecycleState::RefreshPending { editor, .. } => editor.ready.identity,
            LifecycleState::Unmounting { retired, .. } => retired,
        };
        self.state = LifecycleState::Unavailable {
            retired: Some(retired),
        };
        true
    }
}
