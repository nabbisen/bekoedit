//! Task 023: a third WebView run, covering the manual-walkthrough
//! supplement's §B (workspace-tree row and backlink clicks) and §C (Form
//! mode, mouse click on the Text tab) -- the two checks neither RFC-041's
//! nor RFC-044's runs can drive, because both dispatch synthetic,
//! script-generated events (`isTrusted: false`), and a browser withholds
//! its default focus action from those. A real mouse click is
//! indistinguishable from one dispatched at the X level below the
//! browser (XTEST, via `xdotool`) -- `isTrusted: true`, default actions
//! run -- so this run sends every click that way instead.
//!
//! §C's phase spent sixteen real CI runs never completing, chased through
//! window focus, click cadence, rect stability, settle gates, and two
//! genuine harness defects (blocking the event loop; an unjoined eval --
//! both fixed regardless, see `xtest.rs`) before the actual cause turned
//! up in `phase.rs`'s own doc comment: the terminal phase's `as_str()`
//! was sending its *result-stage* name instead of its own phase name, a
//! request `trusted_click_driver.js` rejected before its own `try`, with
//! nothing ever sent back -- indistinguishable, from the outside, from
//! every real hypothesis this task chased. See phase.rs's own doc comment
//! and the review request/review pair dated 2026-09-23 for the full
//! account.
//!
//! A separate run from both `webview_smoke.rs`'s RFC-041 regression and
//! `shell_behaviour.rs`'s RFC-044 coverage, per task 023 §3 (advisory,
//! argued in the review request): its own launch flag
//! (`--webview-trusted-click`), its own driver JS
//! (`trusted_click_driver.js`), its own phase set (`trusted_click::phase`).
//! It reuses `super::transport`'s evaluator-pin handshake for phase
//! semantics -- proven, and the one thing every run must not reimplement
//! (`transport.rs`'s own audit comment) -- but fetches a click target's
//! bounding rect through a separate, bespoke one-shot `document::eval`
//! (`locate_click_target`), the same one-shot pattern already used in
//! production by `source_sync::focus::arm_focus_guard`. This keeps
//! `transport.rs`'s shared `PhaseMessage`/`PhaseCompletion` structs, used
//! by every other (blocking) run, untouched.
//!
//! `trusted_click_driver.js` never calls `.click()` or dispatches a
//! synthetic `MouseEvent`, for setup or anything else: every click in this
//! run, without exception, is a real XTEST click `xtest::click_via_xtest`
//! sends through `xdotool` between two phase exchanges, mirroring
//! `shell_behaviour.rs`'s own precedent for a Rust-side effect between
//! exchanges (`writes_conflict_after`). The driver only watches for what a
//! click already did.
//!
//! The fixture is two files: `parent.md` links to `child.md`, so opening
//! `child.md` and the backlinks panel shows `parent.md` as its one
//! backlink (`bekoedit_fs::find_backlinks`). No document is open at
//! launch -- `reopen_last_workspace` (RFC-043) only repopulates the tree,
//! exactly as `shell_behaviour.rs`'s own contract 7 (`EnterOpens`) relies
//! on -- so `TreeRowFocus`'s click is a real, first-ever open, not a
//! reopen of whatever the harness starts with.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::source_sync::SourceSyncState;

use super::transport::{
    self, CompletedProbe, DriverResult, MessageKind, PhaseMessage, PinnedExchange,
    SMOKE_PROTOCOL_VERSION, reject_with_reason,
};

mod diagnose;
pub(super) use diagnose::record_source_trace;

mod phase;
use phase::{EXPECTED_MILESTONES, TERMINAL_STAGE, TrustedClickPhase};

mod seed;
pub(in crate::webview_smoke) use seed::prepare;

mod settle;
use settle::{SETTLE_GATE, unsettled_lifecycle_state, wait_until_settled};

pub(in crate::webview_smoke) mod xtest;
use xtest::perform_trusted_clicks;

const MARKER: &str = "TASK023_TRUSTED_CLICK_MARKER";
const PHASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

const TRUSTED_CLICK_JS: &str = include_str!("trusted_click_driver.js");

#[derive(Debug)]
struct TrustedClickMachine {
    current: TrustedClickPhase,
    last_applied_exchange_id: Option<u64>,
}

impl TrustedClickMachine {
    const fn new() -> Self {
        Self {
            current: TrustedClickPhase::ProofOfTrust,
            last_applied_exchange_id: None,
        }
    }

    const fn current(&self) -> TrustedClickPhase {
        self.current
    }

    const fn for_phase(current: TrustedClickPhase) -> Self {
        Self {
            current,
            last_applied_exchange_id: None,
        }
    }

    fn validate(
        &self,
        message: &PhaseMessage,
        exchange_id: u64,
        release: Option<PinnedExchange<TrustedClickPhase>>,
    ) -> Result<(), String> {
        // 2026-09-23 review (root-cause-fixed) §2: `failEarly` sends a
        // terminal failure carrying its own reason, but always with
        // `releasedExchangeId: null` -- it has no way to know what Rust
        // actually expected released -- so the pin-release check below
        // rejected it with a generic structural complaint, discarding the
        // driver's own, more specific explanation. A structural rejection
        // now carries that explanation along, when the message has one.
        // Task 025 §2.3 generalised this to a shared helper
        // (`transport::reject_with_reason`), used by every validator.
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
                if self.current.next().is_none() {
                    return Err(format!(
                        "{} phase cannot return nonterminal progress",
                        self.current.as_str()
                    ));
                }
                let expected = self.current.expected_milestone();
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

    fn apply_completed(&mut self, exchange_id: u64, message: &PhaseMessage) -> Result<(), String> {
        if self
            .last_applied_exchange_id
            .is_some_and(|last| exchange_id <= last)
        {
            return Err("driver completion was stale or already applied".into());
        }
        self.last_applied_exchange_id = Some(exchange_id);
        if message.kind == MessageKind::Progress
            && let Some(next) = self.current.next()
        {
            self.current = next;
        }
        Ok(())
    }
}

fn validate_trusted_click_result(result: &DriverResult) -> Result<(), String> {
    if !result.ok {
        return Err(format!(
            "driver failed at {}: {}",
            result.stage,
            result.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if result.stage != TERMINAL_STAGE || result.marker != MARKER {
        return Err("driver returned the wrong terminal stage or marker".into());
    }
    if result.error_toast_seen {
        return Err("an error toast appeared during the trusted-click sequence".into());
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

#[derive(Debug, Default)]
pub struct TrustedClickTerminal {
    state: AtomicU8,
}

impl TrustedClickTerminal {
    fn accept(&self, result: &DriverResult) -> Result<(), String> {
        validate_trusted_click_result(result)?;
        self.state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "trusted-click terminal result was already recorded".to_string())?;
        Ok(())
    }

    pub(super) fn succeeded(&self) -> bool {
        self.state.load(Ordering::SeqCst) == 1
    }
}

/// Adapts the shared transport to this run's own phase semantics, the
/// same shape as `shell_behaviour.rs`'s own adapter.
async fn run_trusted_click_phase(
    phase: TrustedClickPhase,
    exchange_id: u64,
    release: Option<PinnedExchange<TrustedClickPhase>>,
) -> Result<CompletedProbe<TrustedClickPhase>, String> {
    transport::run_driver_phase(TRUSTED_CLICK_JS, phase, exchange_id, release, |message| {
        TrustedClickMachine::for_phase(phase).validate(message, exchange_id, release)
    })
    .await
}

async fn run_trusted_click_sequence(
    desktop: &DesktopContext,
    terminal: &TrustedClickTerminal,
    mut busy_state: impl FnMut() -> Option<String>,
    mut describe_app: impl FnMut() -> String,
) -> Result<DriverResult, String> {
    let mut machine = TrustedClickMachine::new();
    let mut exchange_id = 1_u64;
    let mut release = None;
    // A phase's click(s) happen exactly once, on the exchange that first
    // requests it -- not on every `Pending` retry while polling for that
    // click's outcome. `machine.current()` does not change across
    // retries, so this is the same "only once" shape as
    // `shell_behaviour.rs`'s own `writes_conflict_after` guard.
    let mut clicked_for: Option<TrustedClickPhase> = None;
    loop {
        let phase = machine.current();
        // task 021's settle gate (`settle.rs`): CI's eighth real run
        // showed a DOM-level wait alone is not enough -- the DOM can look
        // ready before SourceSyncState's own async lifecycle has actually
        // finished. No exchange is requested, and no click sent, while the
        // controller would answer Busy.
        wait_until_settled(phase, SETTLE_GATE, &mut busy_state).await?;
        if clicked_for != Some(phase) {
            perform_trusted_clicks(desktop, phase).await?;
            diagnose::note(&format!("{}: click sent", phase.as_str()));
            clicked_for = Some(phase);
        }
        // Timed and logged unconditionally, success or failure -- this is
        // what let task 023's investigation pin the hang to an exact,
        // reproducible 5.001s wall (the shared transport's own
        // round-trip timeout, `transport.rs`, untouched by this run)
        // before the actual cause (phase.rs's own doc comment) turned
        // up. Kept at negligible cost.
        let started = tokio::time::Instant::now();
        let outcome = run_trusted_click_phase(phase, exchange_id, release).await;
        println!(
            "  {} exchange {exchange_id} took {:?}, ok={}",
            phase.as_str(),
            started.elapsed(),
            outcome.is_ok()
        );
        let completed = outcome?;
        machine.validate(&completed.message, exchange_id, release)?;
        transport::validate_completion(
            &completed.completion,
            exchange_id,
            machine.current().as_str(),
            completed.message.kind,
        )?;
        machine.apply_completed(exchange_id, &completed.message)?;
        release = Some(completed.pin);
        if let Some(mut result) = completed.message.result {
            // Task 029: a failure carries what the app knew, not only the page.
            diagnose::enrich_failure(&mut result, &describe_app());
            terminal.accept(&result)?;
            return Ok(result);
        }
        exchange_id = exchange_id
            .checked_add(1)
            .ok_or_else(|| "exchange id exhausted".to_string())?;
        tokio::time::sleep(PHASE_POLL_INTERVAL).await;
    }
}

#[component]
pub fn WebViewTrustedClickDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let sync = use_context::<Signal<SourceSyncState>>();
    let app_state = use_context::<Signal<AppState>>();
    let mode = use_context::<Signal<EditorMode>>();
    let terminal = super::launch_config()
        .trusted_click
        .clone()
        .expect("trusted-click driver requires terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            diagnose::start_clock();
            let describe_app = diagnose::describer(app_state, mode, sync);
            println!("bekoedit task 023 trusted-click run: §B/§C via real XTEST clicks");
            // A peek, not a read: the gate must not subscribe this
            // component to the controller (`shell_behaviour.rs`'s own
            // driver does the same). Reads every non-`Ready` lifecycle
            // state (`settle::unsettled_lifecycle_state`), not
            // `busy_lifecycle_state` -- see that function's own doc
            // comment for why the narrower, queue-gated check cannot see
            // this run's own shape of busy.
            let busy_state = move || match sync.try_peek() {
                Ok(state) => unsettled_lifecycle_state(&state),
                Err(_) => Some("borrowed elsewhere".to_string()),
            };
            match run_trusted_click_sequence(&desktop, &terminal, busy_state, describe_app).await {
                Ok(result) => {
                    for milestone in &result.milestones {
                        println!("  ✓ {milestone}");
                    }
                    println!("bekoedit task 023 trusted-click run PASSED");
                }
                Err(error) => eprintln!("bekoedit task 023 trusted-click run FAILED: {error}"),
            }
            desktop.close();
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests;
