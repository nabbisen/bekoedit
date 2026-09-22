use bekoedit_core::AppState;
use bekoedit_ui_contract::{
    EditorMode,
    source_editor::{BridgeFailureReason, EditorIdentity, SourceEditorId},
};

use super::{DiscardReason, EditorMountHandle, SourceCommand, SourceSyncState};
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
    pub fn is_ready(&self, editor_id: SourceEditorId, document_id: u64) -> bool {
        self.lifecycle.ready_editor().is_some_and(|editor| {
            editor.identity.editor_id == editor_id && editor.identity.document_id == document_id
        })
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self.lifecycle.state, LifecycleState::Unavailable { .. })
    }

    /// The lifecycle state that holds a command in the queue rather than
    /// running it, for a command that names the current document; `None` when
    /// the controller would act on it at once. Read-only: it mirrors `gate` in
    /// `queue.rs` and changes nothing (task 021's settle gate reads it).
    ///
    /// `Mounting` and `Initializing` are reported only while a command is
    /// already waiting in them, as before the queue existed, so the harness's
    /// gate does not start waiting out every mount. `Unavailable` is not
    /// reported: it is answered at once, and waiting does not resolve it.
    ///
    /// Exhaustive on purpose: a new `LifecycleState` must be classified here
    /// before it compiles.
    pub fn busy_lifecycle_state(&self) -> Option<&'static str> {
        match &self.lifecycle.state {
            LifecycleState::Unmounted
            | LifecycleState::Ready(_)
            | LifecycleState::Unavailable { .. } => None,
            LifecycleState::Mounting { .. } => (!self.queue.is_empty()).then_some("Mounting"),
            LifecycleState::Initializing { .. } => {
                (!self.queue.is_empty()).then_some("Initializing")
            }
            LifecycleState::SnapshotPending { .. } => Some("SnapshotPending"),
            LifecycleState::BarrierHeld { .. } => Some("BarrierHeld"),
            LifecycleState::ResumePending { .. } => Some("ResumePending"),
            LifecycleState::RefreshPending { .. } => Some("RefreshPending"),
            LifecycleState::Unmounting { .. } => Some("Unmounting"),
        }
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

    pub fn drain_actions(&mut self) -> Vec<super::ControllerAction> {
        std::mem::take(&mut self.actions)
    }

    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

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
        self.discard_queue(DiscardReason::RelayLost);
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

    /// Whether `command` switches to the source editor the app is already
    /// heading for right now, so that submitting it is a silent no-op
    /// (RFC-047 §5.6, amended by task 022).
    ///
    /// **A queued switch, of any target, is never a no-op.** Something is
    /// already waiting to change the mode, so a further submission must reach
    /// the queue rather than be dropped here -- whether it names a different
    /// target, which must override the stale one (RFC-047 slice 1's own
    /// coalescing test: a save in flight, Preview queued, then Text must
    /// still replace Preview rather than being read as "already Text"), or
    /// the *same* target, which must still replace it, to carry the newer
    /// click's focus token rather than leaving the original -- possibly
    /// already superseded and cancelled -- token to run alone (task 022 case
    /// B).
    ///
    /// Otherwise, "heading for" is one in flight (`SnapshotPending` and
    /// `BarrierHeld` carry their `command`) -- whatever it targets, including
    /// Preview or Form, which replaces the mounted editor as the answer
    /// rather than merely overriding it when the target happens to be a
    /// source editor -- else the mounted editor. Comparing only with the
    /// mounted editor called "Text again" a no-op while Preview was on its
    /// way, and Preview then won: the user's last click lost, with nothing
    /// shown (RFC-047 slice 1's own defect, task 022 case A).
    ///
    /// This is the one notion of "where the app is heading" both this
    /// controller and the focus layer (`focus.rs`'s `claims_focus`) compare
    /// against (task 022 §4): a second, slightly different copy in the focus
    /// layer, built from the UI mode signal alone, was the defect that task
    /// fixed. It is `pub` for exactly that: a method, not a duplicated rule.
    ///
    /// Preview and Form are not source editors, so a switch to either is
    /// never a no-op.
    pub fn is_same_source_mode(&self, command: &SourceCommand) -> bool {
        if self.has_queued_mode_switch() {
            return false;
        }
        let heading_for = match self.in_flight_mode_switch() {
            Some(mode) => source_editor_named(mode),
            None => self.mounted_source_editor(),
        };
        names_source_editor(command, heading_for)
    }

    fn has_queued_mode_switch(&self) -> bool {
        self.queue
            .iter()
            .any(|queued| matches!(queued.command, SourceCommand::SwitchMode(_)))
    }

    /// The mode of a `SwitchMode` already in flight (`SnapshotPending` and
    /// `BarrierHeld` carry their `command`), if any.
    fn in_flight_mode_switch(&self) -> Option<EditorMode> {
        match &self.lifecycle.state {
            LifecycleState::SnapshotPending { command, .. }
            | LifecycleState::BarrierHeld { command, .. } => switch_target(command),
            _ => None,
        }
    }

    fn mounted_source_editor(&self) -> Option<SourceEditorId> {
        self.current_physical_identity()
            .map(|identity| identity.editor_id)
    }
}

fn switch_target(command: &SourceCommand) -> Option<EditorMode> {
    match command {
        SourceCommand::SwitchMode(mode) => Some(*mode),
        _ => None,
    }
}

/// Exhaustive: a new `EditorMode` must say whether it is a source editor.
fn source_editor_named(mode: EditorMode) -> Option<SourceEditorId> {
    match mode {
        EditorMode::Text => Some(SourceEditorId::Text),
        EditorMode::Split => Some(SourceEditorId::Split),
        EditorMode::Preview | EditorMode::Form => None,
    }
}

/// Whether `command` names exactly the source editor `heading_for`.
fn names_source_editor(command: &SourceCommand, heading_for: Option<SourceEditorId>) -> bool {
    let Some(named) = switch_target(command).and_then(source_editor_named) else {
        return false;
    };
    heading_for == Some(named)
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
