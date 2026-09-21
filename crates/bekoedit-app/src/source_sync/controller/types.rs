use bekoedit_ui_contract::source_editor::{
    EditorIdentity, EditorInstanceId, OperationId, SourceEditorId,
};

use std::collections::VecDeque;

use super::super::SourceCommand;
use super::super::lifecycle::LifecycleReducer;
use super::super::lifecycle::{LifecycleEffect, MOUNT_DEADLINE_MS};
use super::interaction::FocusInteraction;

/// How many user commands may wait behind a lifecycle transition (RFC-047
/// §5.2). Two covers an ordinary two-click sequence; more only stores intent
/// old enough to be stale.
pub const QUEUE_DEPTH: usize = 2;

/// How long a queued command may wait, from its acceptance (RFC-047 §5.3): one
/// second past the longest lifecycle deadline it can legitimately wait behind.
/// `BarrierHeld` has no deadline of its own, so this is what bounds it.
pub const QUEUE_ENTRY_DEADLINE_MS: u64 = MOUNT_DEADLINE_MS + 1_000;

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

/// What a queued command is tied to (RFC-047 §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueueScope {
    /// The command means the same thing whenever it runs.
    Anywhere,
    /// The command acts on the open document, recorded at acceptance. It runs
    /// only while that document is still the open one.
    Document(Option<u64>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QueuedCommand {
    pub(super) command: SourceCommand,
    pub(super) focus_token: Option<u64>,
    pub(super) deadline_ms: u64,
    pub(super) scope: QueueScope,
}

/// Why a queued command left the queue without running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardReason {
    /// The queue was full; the newest command was refused.
    Overflow,
    /// It waited longer than `QUEUE_ENTRY_DEADLINE_MS`.
    Expired,
    /// A document-scoped command whose document is no longer the open one.
    DocumentChanged,
    /// The editor became unavailable, and waiting cannot fix that.
    EditorUnavailable,
    /// The relay to the page was lost.
    RelayLost,
    /// The application is shutting down.
    Shutdown,
}

/// A queued command that will not run, and why. It leaves the controller
/// through `SourceSyncState::drain_discards`, so no removal is silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueDiscard {
    pub command: SourceCommand,
    pub reason: DiscardReason,
    /// The focus interaction the command carried, if any. The controller does
    /// not cancel it; the caller decides, as it does for a timed-out command.
    pub focus_token: Option<u64>,
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
    /// Accepted while the editor is mounting; runs when it is ready.
    WaitingForReady,
    /// Accepted behind a lifecycle transition; runs when it ends, or is
    /// reported through `drain_discards` if it cannot.
    Queued,
    /// Refused: the queue is full. Reported through `drain_discards`.
    QueueFull,
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
    pub(super) queue: VecDeque<QueuedCommand>,
    pub(super) discards: Vec<QueueDiscard>,
    pub(super) protected_focus_token: Option<u64>,
    pub(super) bundle_probe_started: bool,
    pub(super) expected_relay_generation: Option<u64>,
    pub(super) relay_generation: Option<u64>,
    pub(super) next_focus_token: u64,
    pub(super) provisional_focus: Option<FocusInteraction>,
    pub(super) pending_focus: Option<FocusInteraction>,
    pub(super) shell_focus_held: bool,
}
