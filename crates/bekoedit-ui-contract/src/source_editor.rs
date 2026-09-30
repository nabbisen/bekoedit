//! Typed CodeMirror lifecycle protocol (RFC-041).

use serde::{Deserialize, Serialize};

use crate::BRIDGE_SCHEMA_VERSION;

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

opaque_id!(EditorInstanceId);
opaque_id!(SourceEpoch);
opaque_id!(OperationId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceEditorId {
    Text,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorIdentity {
    pub instance_id: EditorInstanceId,
    pub editor_id: SourceEditorId,
    pub document_id: u64,
    pub epoch: SourceEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoverPermit {
    pub retired_instance_id: EditorInstanceId,
    pub replacement_instance_id: EditorInstanceId,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SourceEditorRequest {
    ProbeBundle {
        protocol_version: u32,
        operation_id: OperationId,
    },
    InstallRelay {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
    },
    InitEditor {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        container_id: String,
        revision: u64,
        text: String,
        takeover: Option<TakeoverPermit>,
    },
    RequestSnapshot {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
    },
    ResumeEditing {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        snapshot_operation_id: OperationId,
        revision: u64,
    },
    ApplyDocument {
        protocol_version: u32,
        operation_id: OperationId,
        old_identity: EditorIdentity,
        new_epoch: SourceEpoch,
        revision: u64,
        text: String,
    },
    DestroyEditor {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
    },
    /// RFC-046: the converted (or refused) result of one paste, in reply to a
    /// `PasteRequested` event. Outside RFC-041's mount/snapshot/resume state
    /// machine: it is delivered the same way, but carries no `operation_id` and
    /// expects no acknowledgement back through it, since the page applies the
    /// result and moves on -- see `paste.js`.
    PasteResult {
        protocol_version: u32,
        identity: EditorIdentity,
        token: u64,
        outcome: PasteOutcome,
    },
}

/// RFC-046 §3.2. Mirrors `bekoedit_paste::Outcome`, plus `table_no_gfm_form`
/// (§3.4), which the page needs to decide the one extra notice `bekoedit_paste`
/// itself does not know about (it has no `bekoedit_markdown` dependency).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PasteOutcome {
    /// Insert `markdown`. If `table_no_gfm_form`, also raise that notice.
    Converted {
        markdown: String,
        table_no_gfm_form: bool,
    },
    /// Insert the plain flavour the page already holds locally, and raise the
    /// notice named by `reason`.
    Fallback { reason: PasteFallbackReason },
    /// Insert the plain flavour the page already holds locally; no notice.
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteFallbackReason {
    TooLarge,
    Failed,
    TimedOut,
}

impl SourceEditorRequest {
    pub fn protocol_version(&self) -> u32 {
        match self {
            Self::ProbeBundle {
                protocol_version, ..
            }
            | Self::InstallRelay {
                protocol_version, ..
            }
            | Self::InitEditor {
                protocol_version, ..
            }
            | Self::RequestSnapshot {
                protocol_version, ..
            }
            | Self::ResumeEditing {
                protocol_version, ..
            }
            | Self::ApplyDocument {
                protocol_version, ..
            }
            | Self::DestroyEditor {
                protocol_version, ..
            }
            | Self::PasteResult {
                protocol_version, ..
            } => *protocol_version,
        }
    }

    pub fn current_probe(operation_id: OperationId) -> Self {
        Self::ProbeBundle {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BridgeFailureReason {
    UnsupportedVersion,
    MissingContainer,
    EditorUnavailable,
    IdentityMismatch,
    InstanceAlreadyActive,
    CompositionActive,
    RelayUnavailable,
    BridgeError,
}

macro_rules! diagnostic_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

diagnostic_enum!(FocusGuardOutcome {
    Accepted => "accepted",
    Rejected => "rejected",
});
diagnostic_enum!(FocusGuardReason {
    Accepted => "accepted",
    RequestMissing => "requestMissing",
    IdentityMismatch => "identityMismatch",
    MissingGuard => "missingGuard",
    TokenMismatch => "tokenMismatch",
    DivertedPointer => "divertedPointer",
    DivertedTab => "divertedTab",
    DivertedFocusIn => "divertedFocusIn",
    FingerprintMismatch => "fingerprintMismatch",
    ActiveElementIneligible => "activeElementIneligible",
});
diagnostic_enum!(FocusGuardTokenRelation {
    Match => "match",
    Mismatch => "mismatch",
    NoGuard => "noGuard",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardDiversion {
    None => "none",
    Pointer => "pointer",
    Tab => "tab",
    FocusIn => "focusIn",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardFingerprintRelation {
    Equal => "equal",
    Mismatch => "mismatch",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardOriginConnection {
    Connected => "connected",
    Disconnected => "disconnected",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardActiveElementRelation {
    Origin => "origin",
    Body => "body",
    Other => "other",
    None => "none",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardRemovalPolicy {
    LaunchMayBeRemoved => "launchMayBeRemoved",
    LaunchMustRemain => "launchMustRemain",
    NotEvaluated => "notEvaluated",
});
diagnostic_enum!(FocusGuardFallback {
    Eligible => "eligible",
    Ineligible => "ineligible",
    NotEvaluated => "notEvaluated",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusGuardDiagnostic {
    pub outcome: FocusGuardOutcome,
    pub reason: FocusGuardReason,
    pub token_relation: FocusGuardTokenRelation,
    pub diversion: FocusGuardDiversion,
    pub fingerprint_relation: FocusGuardFingerprintRelation,
    pub origin_connection: FocusGuardOriginConnection,
    pub active_element_relation: FocusGuardActiveElementRelation,
    pub removal_policy: FocusGuardRemovalPolicy,
    pub removed_body_fallback: FocusGuardFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SourceEditorEvent {
    BundleReady {
        protocol_version: u32,
        operation_id: OperationId,
    },
    BundleFailed {
        protocol_version: u32,
        operation_id: OperationId,
        reason: BridgeFailureReason,
    },
    RelayReady {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
    },
    RelayFailed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        reason: BridgeFailureReason,
    },
    EditorReady {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        revision: u64,
        reused: bool,
    },
    InitFailed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        reason: BridgeFailureReason,
    },
    Change {
        protocol_version: u32,
        identity: EditorIdentity,
        seq: u64,
        text: String,
        composing: bool,
    },
    Snapshot {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        seq: u64,
        text: String,
        composing: bool,
    },
    SnapshotBlocked {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        reason: BridgeFailureReason,
    },
    EditingResumed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        snapshot_operation_id: OperationId,
        revision: u64,
        was_held: bool,
    },
    ResumeFailed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        snapshot_operation_id: OperationId,
        reason: BridgeFailureReason,
    },
    DocumentApplied {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        revision: u64,
    },
    ApplyDocumentFailed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        reason: BridgeFailureReason,
    },
    Destroyed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
    },
    DestroyFailed {
        protocol_version: u32,
        operation_id: OperationId,
        identity: EditorIdentity,
        reason: BridgeFailureReason,
    },
    Trace {
        protocol_version: u32,
        instance_id: Option<EditorInstanceId>,
        event: String,
        #[serde(default)]
        focus_token: Option<u64>,
        #[serde(default)]
        focus_guard_diagnostic: Option<FocusGuardDiagnostic>,
    },
    /// RFC-046 §3.2: a `paste` in the editor carried `text/html`, and plain
    /// paste was not requested. `html` is `None` when the page already decided
    /// `TooLarge` itself, from the flavour's length alone (§3.1): the page has
    /// already inserted the plain flavour by the time this arrives, so it asks
    /// only for the notice, and expects no `PasteResult` reply.
    PasteRequested {
        protocol_version: u32,
        identity: EditorIdentity,
        token: u64,
        html: Option<String>,
        plain_length: u64,
    },
    /// RFC-046 §3.3: a `PasteResult` arrived for a paste the page can no
    /// longer apply -- the editor's identity changed, or it is held -- so the
    /// page discarded it instead of inserting into the wrong document. Fired
    /// purely to raise the one Warning notice §3.3 requires; there is nothing
    /// further for Rust to compute.
    PasteDiscarded {
        protocol_version: u32,
        identity: EditorIdentity,
        token: u64,
    },
}

impl SourceEditorEvent {
    pub fn protocol_version(&self) -> u32 {
        match self {
            Self::BundleReady {
                protocol_version, ..
            }
            | Self::BundleFailed {
                protocol_version, ..
            }
            | Self::RelayReady {
                protocol_version, ..
            }
            | Self::RelayFailed {
                protocol_version, ..
            }
            | Self::EditorReady {
                protocol_version, ..
            }
            | Self::InitFailed {
                protocol_version, ..
            }
            | Self::Change {
                protocol_version, ..
            }
            | Self::Snapshot {
                protocol_version, ..
            }
            | Self::SnapshotBlocked {
                protocol_version, ..
            }
            | Self::EditingResumed {
                protocol_version, ..
            }
            | Self::ResumeFailed {
                protocol_version, ..
            }
            | Self::DocumentApplied {
                protocol_version, ..
            }
            | Self::ApplyDocumentFailed {
                protocol_version, ..
            }
            | Self::Destroyed {
                protocol_version, ..
            }
            | Self::DestroyFailed {
                protocol_version, ..
            }
            | Self::Trace {
                protocol_version, ..
            }
            | Self::PasteRequested {
                protocol_version, ..
            }
            | Self::PasteDiscarded {
                protocol_version, ..
            } => *protocol_version,
        }
    }

    pub fn has_supported_version(&self) -> bool {
        self.protocol_version() == BRIDGE_SCHEMA_VERSION
    }
}
