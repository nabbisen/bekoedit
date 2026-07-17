use serde::{Deserialize, Serialize};

use super::{EXPECTED_MILESTONES, MARKER};

pub(super) const SMOKE_PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DriverResult {
    pub(super) ok: bool,
    pub(super) stage: String,
    pub(super) marker: String,
    pub(super) milestones: Vec<String>,
    pub(super) error_toast_seen: bool,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum MessageKind {
    Pending,
    Progress,
    Terminal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PhaseMessage {
    pub(super) protocol_version: u32,
    pub(super) exchange_id: u64,
    pub(super) kind: MessageKind,
    pub(super) phase: String,
    pub(super) released_exchange_id: Option<u64>,
    pub(super) released_phase: Option<String>,
    pub(super) milestone: Option<String>,
    pub(super) result: Option<DriverResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PhaseRequest<'a> {
    pub(super) protocol_version: u32,
    pub(super) exchange_id: u64,
    pub(super) phase: &'a str,
    pub(super) release_exchange_id: Option<u64>,
    pub(super) release_phase: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PhaseAcknowledgement<'a> {
    pub(super) protocol_version: u32,
    pub(super) exchange_id: u64,
    pub(super) phase: &'a str,
    pub(super) kind: MessageKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PhaseCompletion {
    pub(super) protocol_version: u32,
    pub(super) exchange_id: u64,
    pub(super) phase: String,
    pub(super) kind: MessageKind,
    pub(super) acknowledgement_processed: bool,
    pub(super) evaluator_pinned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SmokePhase {
    Launch,
    Editor,
    Preview,
}

impl SmokePhase {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::Editor => "editor",
            Self::Preview => "preview",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PinnedExchange {
    pub(super) exchange_id: u64,
    pub(super) phase: SmokePhase,
}

#[derive(Debug)]
pub(super) struct CompletedProbe {
    pub(super) message: PhaseMessage,
    pub(super) completion: PhaseCompletion,
    pub(super) pin: PinnedExchange,
}

#[derive(Debug)]
pub(super) struct PhaseMachine {
    pub(super) current: SmokePhase,
    last_applied_exchange_id: Option<u64>,
}

impl PhaseMachine {
    pub(super) const fn new() -> Self {
        Self {
            current: SmokePhase::Launch,
            last_applied_exchange_id: None,
        }
    }

    pub(super) const fn current(&self) -> SmokePhase {
        self.current
    }

    pub(super) const fn for_phase(current: SmokePhase) -> Self {
        Self {
            current,
            last_applied_exchange_id: None,
        }
    }

    pub(super) fn validate(
        &self,
        message: &PhaseMessage,
        exchange_id: u64,
        release: Option<PinnedExchange>,
    ) -> Result<(), String> {
        if message.protocol_version != SMOKE_PROTOCOL_VERSION {
            return Err("driver returned an unsupported smoke protocol version".into());
        }
        if message.exchange_id != exchange_id {
            return Err("driver returned the wrong smoke exchange".into());
        }
        if message.phase != self.current.as_str() {
            return Err("driver returned an out-of-order phase".into());
        }
        let released_matches = match release {
            Some(release) => {
                message.released_exchange_id == Some(release.exchange_id)
                    && message.released_phase.as_deref() == Some(release.phase.as_str())
            }
            None => message.released_exchange_id.is_none() && message.released_phase.is_none(),
        };
        if !released_matches {
            return Err("driver did not release the exact prior evaluator pin".into());
        }
        match message.kind {
            MessageKind::Pending => {
                if message.milestone.is_some() || message.result.is_some() {
                    return Err("pending driver message contained progress data".into());
                }
            }
            MessageKind::Progress => {
                let expected = match self.current {
                    SmokePhase::Launch => "new_clicked",
                    SmokePhase::Editor => "preview_clicked",
                    SmokePhase::Preview => {
                        return Err("preview phase cannot return nonterminal progress".into());
                    }
                };
                if message.milestone.as_deref() != Some(expected) || message.result.is_some() {
                    return Err("driver returned malformed phase progress".into());
                }
            }
            MessageKind::Terminal => {
                if message.milestone.is_some() || message.result.is_none() {
                    return Err("terminal driver message was malformed".into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_completed(
        &mut self,
        exchange_id: u64,
        message: &PhaseMessage,
    ) -> Result<(), String> {
        if self
            .last_applied_exchange_id
            .is_some_and(|last| exchange_id <= last)
        {
            return Err("driver completion was stale or already applied".into());
        }
        self.last_applied_exchange_id = Some(exchange_id);
        if message.kind == MessageKind::Progress {
            self.current = match self.current {
                SmokePhase::Launch => SmokePhase::Editor,
                SmokePhase::Editor | SmokePhase::Preview => SmokePhase::Preview,
            };
        }
        Ok(())
    }
}

pub(super) fn validate_driver_result(result: &DriverResult) -> Result<(), String> {
    if !result.ok {
        return Err(format!(
            "driver failed at {}: {}",
            result.stage,
            result.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if result.stage != "preview_verified" || result.marker != MARKER {
        return Err("driver returned the wrong terminal stage or marker".into());
    }
    if result.error_toast_seen {
        return Err("an error toast appeared during the WebView smoke sequence".into());
    }
    if result.error.is_some() {
        return Err("successful driver result unexpectedly contained an error".into());
    }
    if result
        .milestones
        .iter()
        .map(String::as_str)
        .ne(EXPECTED_MILESTONES)
    {
        return Err("driver returned an incomplete or out-of-order milestone list".into());
    }
    Ok(())
}

pub(super) fn validate_completion(
    completion: &PhaseCompletion,
    exchange_id: u64,
    phase: &str,
    kind: MessageKind,
) -> Result<(), String> {
    if completion.protocol_version != SMOKE_PROTOCOL_VERSION
        || completion.exchange_id != exchange_id
        || completion.phase != phase
        || completion.kind != kind
        || !completion.acknowledgement_processed
        || !completion.evaluator_pinned
    {
        return Err(format!(
            "{phase} phase evaluator returned invalid pinned completion"
        ));
    }
    Ok(())
}
