use bekoedit_core::AppState;
use bekoedit_ui_contract::{
    EditorMode,
    source_editor::{BridgeFailureReason, EditorIdentity, SourceEditorId},
};

use super::{EditorMountHandle, SourceCommand, SourceSyncState};
use super::{SessionFingerprint, SourceSyncError, TransitionError};
use crate::source_sync::lifecycle::{LifecycleState, MountIntent, ReadyEditor};

pub fn fingerprint(app: &AppState) -> SessionFingerprint {
    let document_id = app.session.as_ref().map(|session| session.document_id);
    SessionFingerprint {
        document_id,
        revision: app.session.as_ref().map(|session| session.revision),
        source_token: document_id.unwrap_or_default(),
    }
}

impl SourceSyncState {
    pub fn relay_generation_started(&mut self, generation: u64) {
        self.expected_relay_generation = Some(generation);
        self.relay_generation = None;
    }

    pub fn relay_generation_ready(&mut self, generation: u64, now_ms: u64) -> bool {
        if self.expected_relay_generation != Some(generation) {
            return false;
        }
        self.relay_generation = Some(generation);
        self.lifecycle.rearm_transport_deadlines(now_ms);
        true
    }

    pub fn relay_generation(&self) -> Option<u64> {
        self.relay_generation
    }

    pub fn relay_disconnected(&mut self, generation: u64) -> bool {
        if self.expected_relay_generation != Some(generation) {
            return false;
        }
        self.expected_relay_generation = None;
        let acknowledged = self.relay_generation == Some(generation);
        if !acknowledged {
            return false;
        }
        self.relay_generation = None;
        self.waiting_command = None;
        self.protected_focus_token = None;
        self.actions.clear();
        if self.lifecycle.abandon_bundle_probe() {
            self.bundle_probe_started = false;
        }
        self.lifecycle.relay_lost()
    }

    pub fn drain_dispatchable_actions(&mut self) -> Vec<super::ControllerAction> {
        if self.relay_generation.is_some() {
            return self.drain_actions();
        }
        let (dispatchable, waiting): (Vec<_>, Vec<_>) =
            self.actions.drain(..).partition(|action| {
                matches!(
                    action,
                    super::ControllerAction::Execute { .. } | super::ControllerAction::Focus { .. }
                )
            });
        self.actions = waiting;
        dispatchable
    }

    pub fn has_dispatchable_actions(&self) -> bool {
        self.actions.iter().any(|action| {
            self.relay_generation.is_some()
                || matches!(
                    action,
                    super::ControllerAction::Execute { .. } | super::ControllerAction::Focus { .. }
                )
        })
    }

    pub(super) fn transport_is_holding_lifecycle_action(&self) -> bool {
        self.relay_generation.is_none()
            && self
                .actions
                .iter()
                .any(|action| matches!(action, super::ControllerAction::Lifecycle(_)))
    }

    pub(super) fn matches_current_mount(&self, intent: &MountIntent) -> bool {
        let matches_identity = |identity: EditorIdentity| {
            identity.editor_id == intent.editor_id && identity.document_id == intent.document_id
        };
        match &self.lifecycle.state {
            LifecycleState::Mounting { identity, .. }
            | LifecycleState::Initializing { identity, .. } => matches_identity(*identity),
            LifecycleState::Ready(editor) | LifecycleState::SnapshotPending { editor, .. } => {
                matches_identity(editor.identity)
            }
            LifecycleState::BarrierHeld { editor, .. }
            | LifecycleState::ResumePending { editor, .. }
            | LifecycleState::RefreshPending { editor, .. } => {
                matches_identity(editor.ready.identity)
            }
            LifecycleState::Unmounting {
                waiting: Some(current),
                ..
            } => current == intent,
            _ => false,
        }
    }

    pub(super) fn current_physical_identity(&self) -> Option<EditorIdentity> {
        match self.lifecycle.state {
            LifecycleState::Mounting { identity, .. }
            | LifecycleState::Initializing { identity, .. }
            | LifecycleState::Ready(ReadyEditor { identity, .. })
            | LifecycleState::SnapshotPending {
                editor: ReadyEditor { identity, .. },
                ..
            } => Some(identity),
            LifecycleState::BarrierHeld { editor, .. }
            | LifecycleState::ResumePending { editor, .. }
            | LifecycleState::RefreshPending { editor, .. } => Some(editor.ready.identity),
            _ => None,
        }
    }

    pub(super) fn owns_handle(&self, handle: EditorMountHandle) -> bool {
        self.current_physical_identity().is_some_and(|identity| {
            identity.instance_id == handle.instance_id
                && identity.editor_id == handle.editor_id
                && identity.document_id == handle.document_id
        })
    }

    pub(super) fn is_same_source_mode(&self, command: &SourceCommand) -> bool {
        let SourceCommand::SwitchMode(target) = command else {
            return false;
        };
        let current = self
            .current_physical_identity()
            .map(|identity| identity.editor_id);
        matches!(
            (current, target),
            (Some(SourceEditorId::Text), EditorMode::Text)
                | (Some(SourceEditorId::Split), EditorMode::Split)
        )
    }
}

impl From<TransitionError> for SourceSyncError {
    fn from(value: TransitionError) -> Self {
        Self::Transition(value)
    }
}

impl From<BridgeFailureReason> for SourceSyncError {
    fn from(value: BridgeFailureReason) -> Self {
        match value {
            BridgeFailureReason::CompositionActive => Self::CompositionActive,
            BridgeFailureReason::IdentityMismatch => Self::IdentityMismatch,
            BridgeFailureReason::UnsupportedVersion => Self::UnsupportedVersion,
            _ => Self::EditorUnavailable,
        }
    }
}
