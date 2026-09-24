use super::transport::{
    MessageKind, PhaseKind, PhaseMessage, PinnedExchange, SMOKE_PROTOCOL_VERSION,
    reject_with_reason,
};
use super::{EXPECTED_MILESTONES, MARKER};

pub(super) use super::transport::{DriverResult, validate_completion};

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

    /// The transition order; `Preview` is terminal. `apply_completed` and the
    /// bijection test walk this same table (task 025 review §3.2).
    pub(super) const fn next(self) -> Option<Self> {
        match self {
            Self::Launch => Some(Self::Editor),
            Self::Editor => Some(Self::Preview),
            Self::Preview => None,
        }
    }
}

impl PhaseKind for SmokePhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}

pub(super) type CompletedProbe = super::transport::CompletedProbe<SmokePhase>;

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
        release: Option<PinnedExchange<SmokePhase>>,
    ) -> Result<(), String> {
        // Task 025 §2.3: a structural rejection carries the driver's own
        // reason along, when the message is a terminal failure that has
        // one (e.g. driver.js's failEarly, which always reports
        // releasedExchangeId: null since it cannot know what Rust actually
        // expected released -- see trusted_click.rs's own validate() for
        // the finding this generalises).
        if message.protocol_version != SMOKE_PROTOCOL_VERSION {
            return Err(reject_with_reason(
                message,
                "driver returned an unsupported smoke protocol version",
            ));
        }
        if message.exchange_id != exchange_id {
            return Err(reject_with_reason(
                message,
                "driver returned the wrong smoke exchange",
            ));
        }
        if message.phase != self.current.as_str() {
            return Err(reject_with_reason(
                message,
                "driver returned an out-of-order phase",
            ));
        }
        let released_matches = match release {
            Some(release) => {
                message.released_exchange_id == Some(release.exchange_id)
                    && message.released_phase.as_deref() == Some(release.phase.as_str())
            }
            None => message.released_exchange_id.is_none() && message.released_phase.is_none(),
        };
        if !released_matches {
            return Err(reject_with_reason(
                message,
                "driver did not release the exact prior evaluator pin",
            ));
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
            self.current = self.current.next().unwrap_or(self.current);
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
