//! The evaluator-pin transport shared by every WebView driver run.
//!
//! Extracted per RFC-044 slice-1 handoff §3: `run_driver_phase`, the
//! pin/exchange types and completion validation are the same handshake for
//! any run, so there is exactly one copy to re-audit against a Dioxus
//! upgrade. What is *not* here -- the phase enum and its transition order,
//! the phase bodies/selectors/assertions, a run's milestone list and
//! terminal-stage validation -- is run-specific semantics and stays with
//! each run.
//!
//! Audited against Dioxus Desktop/Document 0.7.9. `NativeDioxusChannel::close`
//! only clears the JS queue; its `FinalizationRegistry` emits the query drop,
//! whose slab entry owns `DesktopEvaluator`'s generational `Owner`. The
//! smoke-only JS pin keeps that exact channel reachable until this joined
//! return is consumed. Re-audit `native_eval.ts`, `query.rs`, `document.rs`,
//! and `dioxus-document` `eval.rs` before updating Dioxus.

use std::time::Duration;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

pub(super) const SMOKE_PROTOCOL_VERSION: u32 = 2;
const PHASE_EVALUATOR_TIMEOUT: Duration = Duration::from_secs(5);

/// A run's phase enum implements this to plug into the shared transport.
/// `Copy` because a phase is a small tag threaded through several async
/// steps and re-read after each one.
pub(super) trait PhaseKind: Copy {
    fn as_str(self) -> &'static str;
}

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
    /// Task 025 review §3.1: why the part after the acknowledgement failed.
    /// A thrown JS message does not survive `eval.join()`; a returned one does.
    #[serde(default)]
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PinnedExchange<Phase> {
    pub(super) exchange_id: u64,
    pub(super) phase: Phase,
}

#[derive(Debug)]
pub(super) struct CompletedProbe<Phase> {
    pub(super) message: PhaseMessage,
    pub(super) completion: PhaseCompletion,
    pub(super) pin: PinnedExchange<Phase>,
}

/// Runs one phase's evaluator-pin exchange: request, receive the driver's
/// report, let the caller validate it against its own phase semantics
/// (`validate`) before it is acknowledged, acknowledge, then join the
/// pinned completion. `driver_js` is the run's own driver source; the
/// handshake itself is identical for every run.
pub(super) async fn run_driver_phase<Phase, Validate>(
    driver_js: &'static str,
    phase: Phase,
    exchange_id: u64,
    release: Option<PinnedExchange<Phase>>,
    validate: Validate,
) -> Result<CompletedProbe<Phase>, String>
where
    Phase: PhaseKind,
    Validate: FnOnce(&PhaseMessage) -> Result<(), String>,
{
    let phase_name = phase.as_str();
    let deadline = tokio::time::Instant::now() + PHASE_EVALUATOR_TIMEOUT;
    let mut eval = document::eval(driver_js);
    eval.send(PhaseRequest {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id,
        phase: phase_name,
        release_exchange_id: release.map(|pin| pin.exchange_id),
        release_phase: release.map(|pin| pin.phase.as_str()),
    })
    .map_err(|error| format!("could not start {phase_name} phase: {error}"))?;
    let message = tokio::time::timeout_at(deadline, eval.recv::<PhaseMessage>())
        .await
        .map_err(|_| format!("{phase_name} phase evaluator did not report progress"))?
        .map_err(|error| format!("{phase_name} phase evaluator receive failed: {error}"))?;
    validate(&message)?;
    eval.send(PhaseAcknowledgement {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id,
        phase: phase_name,
        kind: message.kind,
    })
    .map_err(|error| format!("could not acknowledge {phase_name} phase: {error}"))?;

    // Audited against Dioxus Desktop/Document 0.7.9. NativeDioxusChannel::close
    // only clears the JS queue; its FinalizationRegistry emits the query drop,
    // whose slab entry owns DesktopEvaluator's generational Owner. The
    // smoke-only JS pin keeps that exact channel reachable until this joined
    // return is consumed. Re-audit native_eval.ts, query.rs, document.rs, and
    // dioxus-document eval.rs before updating Dioxus.
    // Task 025 §2.4: a real CI run (an unknown-phase failEarly rejection on
    // a run's first exchange, no prior pin to release, so validate() above
    // has nothing structural to reject) showed the JS exception's own
    // message does not survive to `{error}` here -- Dioxus/WebKitGTK
    // surfaces only a generic `EvalError::Communication`. But `message`
    // was already received and validated above; when it is itself a
    // terminal failure carrying a reason, that reason is known and is
    // used instead of letting the generic join error discard it.
    let completion = tokio::time::timeout_at(deadline, eval.join::<PhaseCompletion>())
        .await
        .map_err(|_| {
            format!("{phase_name} phase evaluator did not complete after acknowledgement")
        })?
        .map_err(|error| {
            reject_with_reason(
                &message,
                &format!("{phase_name} phase evaluator join failed: {error}"),
            )
        })?;
    Ok(CompletedProbe {
        message,
        completion,
        pin: PinnedExchange { exchange_id, phase },
    })
}

/// Appends a terminal failure's own driver-reported reason to a structural
/// rejection, when `message` carries one -- task 023 review
/// (root-cause-fixed) §2's finding, generalised for every validator
/// (task 025 §2.3): a driver's early-rejection report (e.g. `failEarly`)
/// always carries `releasedExchangeId: null`, since it has no way to know
/// what Rust actually expected released, so a bare structural complaint
/// (below) would otherwise discard the driver's own, more specific reason.
pub(super) fn reject_with_reason(message: &PhaseMessage, base: &str) -> String {
    let reason = (message.kind == MessageKind::Terminal)
        .then_some(message.result.as_ref())
        .flatten()
        .filter(|result| !result.ok)
        .and_then(|result| result.error.as_deref());
    match reason {
        Some(reason) => format!("{base}: {reason}"),
        None => base.to_string(),
    }
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
        || completion.error.is_some()
    {
        let base = format!("{phase} phase evaluator returned invalid pinned completion");
        return Err(match &completion.error {
            Some(reason) => format!("{base}: {reason}"),
            None => base,
        });
    }
    Ok(())
}

/// Walks `next` from `first` to `None`, returning each phase visited, in
/// order -- the one bounded walk every run's tests share (task 025 review
/// §3.3/§4). `bound` is the run's exhaustive phase count: a walk longer than
/// that is a cycle, and fails here, naming the phase, instead of hanging the
/// suite.
#[cfg(test)]
pub(super) fn phases_via_next<Phase: PhaseKind>(
    first: Phase,
    next: impl Fn(Phase) -> Option<Phase>,
    bound: usize,
) -> Vec<Phase> {
    let mut walked = vec![first];
    let mut current = first;
    while let Some(following) = next(current) {
        assert!(
            walked.len() < bound,
            "next() walk exceeded phase_count() ({bound}) without reaching None -- \
             a cycle through {}?",
            following.as_str()
        );
        walked.push(following);
        current = following;
    }
    walked
}

/// Parses a driver's `const phases = [ ... ];` literal array of
/// double-quoted phase names out of its JS source. Shared by every run's
/// bijection test (task 025 §2.2), so there is one copy of this parse
/// rather than three -- each run's own test only supplies its `as_str()`
/// set to compare.
#[cfg(test)]
pub(super) fn parse_js_declared_phase_list(source: &str) -> std::collections::BTreeSet<&str> {
    source
        .split_once("const phases = [")
        .expect("driver must declare its phases array")
        .1
        .split_once(']')
        .expect("phases array must be closed")
        .0
        .split(',')
        .map(|entry| entry.trim().trim_matches('"'))
        .filter(|entry| !entry.is_empty())
        .collect()
}
