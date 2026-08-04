use bekoedit_core::AppState;
use bekoedit_ui_contract::source_editor::SourceEditorEvent;

use super::super::SourceSyncError;
use super::super::lifecycle::{LifecycleState, TransitionError};
use super::support::fingerprint;
use super::types::{EventOutcome, SourceSyncState};

impl SourceSyncState {
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
}
