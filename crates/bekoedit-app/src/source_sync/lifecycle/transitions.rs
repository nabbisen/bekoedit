use super::*;

impl LifecycleReducer {
    pub fn begin_snapshot(
        &mut self,
        command: SourceCommand,
        now_ms: u64,
    ) -> Result<LifecycleEffect, TransitionError> {
        let LifecycleState::Ready(editor) = self.state else {
            return Err(TransitionError::Busy);
        };
        let operation = self.operation(now_ms, SNAPSHOT_DEADLINE_MS);
        self.state = LifecycleState::SnapshotPending {
            editor,
            command,
            operation,
        };
        Ok(LifecycleEffect::RequestSnapshot(
            editor.identity,
            operation.operation_id,
        ))
    }

    pub fn accept_snapshot(
        &mut self,
        event: &SourceEditorEvent,
        accepted_revision: u64,
        before: SessionFingerprint,
    ) -> Result<LifecycleEffect, TransitionError> {
        self.require_version(event)?;
        let LifecycleState::SnapshotPending {
            mut editor,
            command,
            operation,
        } = self.state.clone()
        else {
            return Err(TransitionError::InvalidState);
        };
        let SourceEditorEvent::Snapshot {
            operation_id,
            identity,
            seq,
            composing,
            ..
        } = event
        else {
            return Err(TransitionError::InvalidState);
        };
        if *operation_id != operation.operation_id
            || *identity != editor.identity
            || *composing
            || *seq <= editor.last_seq
            || before.document_id != Some(identity.document_id)
        {
            return Err(TransitionError::Stale);
        }
        editor.last_seq = *seq;
        editor.revision = accepted_revision;
        self.state = LifecycleState::BarrierHeld {
            editor: HeldEditor {
                ready: editor,
                snapshot_operation: *operation_id,
                certainty: HoldCertainty::Confirmed,
            },
            command: command.clone(),
            before,
        };
        Ok(LifecycleEffect::ExecuteCommand(command))
    }

    pub fn handle_snapshot_blocked(
        &mut self,
        event: &SourceEditorEvent,
    ) -> Result<(), TransitionError> {
        self.require_version(event)?;
        let LifecycleState::SnapshotPending {
            editor, operation, ..
        } = self.state.clone()
        else {
            return Err(TransitionError::InvalidState);
        };
        let SourceEditorEvent::SnapshotBlocked {
            operation_id,
            identity,
            reason,
            ..
        } = event
        else {
            return Err(TransitionError::InvalidState);
        };
        if *operation_id != operation.operation_id || *identity != editor.identity {
            return Err(TransitionError::Stale);
        }
        if *reason == bekoedit_ui_contract::source_editor::BridgeFailureReason::CompositionActive {
            self.state = LifecycleState::Ready(editor);
        } else {
            self.state = LifecycleState::Unavailable {
                retired: Some(editor.identity),
            };
        }
        Ok(())
    }

    pub fn reject_snapshot(
        &mut self,
        event: &SourceEditorEvent,
        stream_is_current: bool,
        now_ms: u64,
    ) -> Result<Option<LifecycleEffect>, TransitionError> {
        self.require_version(event)?;
        let LifecycleState::SnapshotPending {
            editor, operation, ..
        } = self.state.clone()
        else {
            return Err(TransitionError::InvalidState);
        };
        let SourceEditorEvent::Snapshot {
            operation_id,
            identity,
            ..
        } = event
        else {
            return Err(TransitionError::InvalidState);
        };
        if *operation_id != operation.operation_id || *identity != editor.identity {
            return Err(TransitionError::Stale);
        }
        let held = HeldEditor {
            ready: editor,
            snapshot_operation: *operation_id,
            certainty: HoldCertainty::Confirmed,
        };
        if stream_is_current {
            Ok(Some(self.start_resume(held, now_ms)))
        } else {
            self.state = LifecycleState::Unavailable {
                retired: Some(editor.identity),
            };
            Ok(None)
        }
    }

    pub fn command_completed(
        &mut self,
        success: bool,
        after: SessionFingerprint,
        now_ms: u64,
    ) -> Result<(CommandDisposition, Option<LifecycleEffect>), TransitionError> {
        let LifecycleState::BarrierHeld {
            editor,
            command,
            before,
        } = self.state.clone()
        else {
            return Err(TransitionError::InvalidState);
        };
        let unchanged = before == after;
        let disposition = if !success {
            if unchanged {
                CommandDisposition::Resume
            } else {
                CommandDisposition::Unavailable
            }
        } else {
            match command {
                SourceCommand::SaveNow | SourceCommand::SaveAs(_) if unchanged => {
                    CommandDisposition::Resume
                }
                SourceCommand::RestoreHistory(_)
                | SourceCommand::MoveSectionUp(_)
                | SourceCommand::MoveSectionDown(_)
                    if after.document_id == before.document_id && after.revision.is_some() =>
                {
                    CommandDisposition::Refresh {
                        revision: after.revision.unwrap_or_default(),
                    }
                }
                SourceCommand::SwitchMode(_)
                | SourceCommand::OpenDocument(_)
                | SourceCommand::NewUntitled
                | SourceCommand::OpenWorkspace(_)
                | SourceCommand::CloseWorkspace => CommandDisposition::Destroy,
                _ => CommandDisposition::Unavailable,
            }
        };
        let effect = match disposition {
            CommandDisposition::Resume => Some(self.start_resume(editor, now_ms)),
            CommandDisposition::Refresh { revision } => {
                Some(self.start_refresh(editor, revision, now_ms))
            }
            CommandDisposition::Destroy => {
                Some(self.start_destroy(editor.ready.identity, None, now_ms)?)
            }
            CommandDisposition::Unavailable => {
                self.state = LifecycleState::Unavailable {
                    retired: Some(editor.ready.identity),
                };
                None
            }
        };
        Ok((disposition, effect))
    }

    pub fn handle_resume_event(
        &mut self,
        event: &SourceEditorEvent,
    ) -> Result<(), TransitionError> {
        self.require_version(event)?;
        let LifecycleState::ResumePending { editor, operation } = self.state else {
            return Err(TransitionError::InvalidState);
        };
        match event {
            SourceEditorEvent::EditingResumed {
                operation_id,
                identity,
                snapshot_operation_id,
                revision,
                ..
            } if *operation_id == operation.operation_id
                && *identity == editor.ready.identity
                && *snapshot_operation_id == editor.snapshot_operation
                && *revision == editor.ready.revision =>
            {
                self.state = LifecycleState::Ready(editor.ready);
                Ok(())
            }
            SourceEditorEvent::ResumeFailed {
                operation_id,
                identity,
                snapshot_operation_id,
                ..
            } if *operation_id == operation.operation_id
                && *identity == editor.ready.identity
                && *snapshot_operation_id == editor.snapshot_operation =>
            {
                self.state = LifecycleState::Unavailable {
                    retired: Some(editor.ready.identity),
                };
                Ok(())
            }
            _ => Err(TransitionError::Stale),
        }
    }

    pub fn handle_document_event(
        &mut self,
        event: &SourceEditorEvent,
    ) -> Result<(), TransitionError> {
        self.require_version(event)?;
        let LifecycleState::RefreshPending {
            editor,
            new_epoch,
            revision,
            operation,
        } = self.state
        else {
            return Err(TransitionError::InvalidState);
        };
        match event {
            SourceEditorEvent::DocumentApplied {
                operation_id,
                identity,
                revision: actual_revision,
                ..
            } if *operation_id == operation.operation_id
                && identity.instance_id == editor.ready.identity.instance_id
                && identity.editor_id == editor.ready.identity.editor_id
                && identity.document_id == editor.ready.identity.document_id
                && identity.epoch == new_epoch
                && *actual_revision == revision =>
            {
                self.state = LifecycleState::Ready(ReadyEditor {
                    identity: *identity,
                    revision,
                    last_seq: 0,
                });
                Ok(())
            }
            SourceEditorEvent::ApplyDocumentFailed {
                operation_id,
                identity,
                ..
            } if *operation_id == operation.operation_id
                && identity.instance_id == editor.ready.identity.instance_id =>
            {
                self.state = LifecycleState::Unavailable {
                    retired: Some(editor.ready.identity),
                };
                Ok(())
            }
            _ => Err(TransitionError::Stale),
        }
    }

    pub fn timeout(
        &mut self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<Option<LifecycleEffect>, TransitionError> {
        if self
            .bundle_probe
            .is_some_and(|pending| Self::expired(pending, operation_id, now_ms))
        {
            self.bundle_probe = None;
            self.bundle_ready = false;
            self.state = LifecycleState::Unavailable { retired: None };
            return Ok(None);
        }
        match self.state.clone() {
            LifecycleState::SnapshotPending {
                editor, operation, ..
            } if Self::expired(operation, operation_id, now_ms) => {
                let held = HeldEditor {
                    ready: editor,
                    snapshot_operation: operation.operation_id,
                    certainty: HoldCertainty::Possible,
                };
                Ok(Some(self.start_resume(held, now_ms)))
            }
            LifecycleState::ResumePending { editor, operation }
            | LifecycleState::RefreshPending {
                editor, operation, ..
            } if Self::expired(operation, operation_id, now_ms) => {
                self.state = LifecycleState::Unavailable {
                    retired: Some(editor.ready.identity),
                };
                Ok(None)
            }
            LifecycleState::Mounting {
                identity, relay, ..
            }
            | LifecycleState::Initializing {
                identity,
                operation: relay,
                ..
            } if Self::expired(relay, operation_id, now_ms) => {
                self.state = LifecycleState::Unavailable {
                    retired: Some(identity),
                };
                Ok(None)
            }
            LifecycleState::Unmounting {
                retired,
                operation,
                waiting,
            } if Self::expired(operation, operation_id, now_ms) => {
                if let Some(intent) = waiting {
                    let replacement = self.allocate_identity(&intent)?;
                    self.next_nonce = self
                        .next_nonce
                        .checked_add(1)
                        .ok_or(TransitionError::InvalidState)?;
                    let permit = TakeoverPermit {
                        retired_instance_id: retired.instance_id,
                        replacement_instance_id: replacement.instance_id,
                        nonce: self.next_nonce,
                    };
                    Ok(Some(self.start_mount_with_identity(
                        intent,
                        replacement,
                        now_ms,
                        Some(permit),
                    )))
                } else {
                    self.state = LifecycleState::Unavailable {
                        retired: Some(retired),
                    };
                    Ok(None)
                }
            }
            _ => Err(TransitionError::Stale),
        }
    }
}
