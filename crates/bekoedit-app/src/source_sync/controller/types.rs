use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEditorId,
};

use super::super::SourceCommand;
use super::super::lifecycle::LifecycleEffect;
use super::super::lifecycle::LifecycleReducer;
use super::interaction::FocusInteraction;

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
pub(super) struct PendingCommand {
    pub(super) command: SourceCommand,
    pub(super) focus_token: Option<u64>,
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
    pub(super) actions: Vec<ControllerAction>,
    pub(super) waiting_command: Option<PendingCommand>,
    pub(super) protected_focus_token: Option<u64>,
    pub(super) bundle_probe_started: bool,
    pub(super) expected_relay_generation: Option<u64>,
    pub(super) relay_generation: Option<u64>,
    pub(super) next_focus_token: u64,
    pub(super) provisional_focus: Option<FocusInteraction>,
    pub(super) pending_focus: Option<FocusInteraction>,
    pub(super) shell_focus_held: bool,
}
