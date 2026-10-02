use std::borrow::Cow;
use std::path::Path;
use std::time::Duration;

use bekoedit_core::AppState;
use bekoedit_ui_contract::{
    EditorMode,
    source_editor::{EditorIdentity, SourceEditorId},
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::toast::Toast;

use super::{
    SourceCommand, SourceSyncState, SubmitOutcome, controller::FocusClaim,
    controller::FocusResolution,
};

const ARM_TIMEOUT: Duration = Duration::from_millis(250);
/// How long the page's own arm script waits for Rust's release (task 039
/// §2.2) before giving up on its own side. Comfortably longer than
/// `ARM_TIMEOUT`, so the ordinary path always resolves via the release, not
/// this bound -- it exists only so a dropped `ARM_TIMEOUT` race (Rust never
/// sends a release because it stopped waiting) cannot leave the page's
/// promise, and the query it keeps reachable, pending forever.
const GUARD_RELEASE_TIMEOUT_MS: u64 = 1000;
const FOCUS_GUARD_BOOTSTRAP: &str = include_str!("../../assets/focus-guard-bundle.js");
const FOCUS_GUARD_PROTOCOL_VERSION: u32 = 2;

/// Shell surfaces call this before moving DOM focus (RFC-042 §6.2 rule 1).
/// It routes through `acquire_shell_focus`, so every caller claims shell
/// focus authority — not just cancels a pending source-focus interaction.
/// Only call this for an actual shell surface with a close path that will
/// call `release_shell_focus` (menus, disclosure panels, screen
/// replacements). For a one-shot native OS dialog that isn't a shell
/// surface at all, use `cancel_pending_source_focus` instead — see the
/// review correction that split these (RFC-042 slice 1 re-review, C1/C2).
pub fn cancel_source_focus(mut sync: Signal<SourceSyncState>) {
    if let Some(token) = sync.write().acquire_shell_focus() {
        cancel_focus_guards_through(token);
    }
}

/// Cancels a pending source-focus interaction without claiming shell focus
/// authority. For call sites that briefly steal OS-level focus (a native
/// file dialog) but have no in-app close path to release authority from —
/// there is no shell surface here for RFC-042 §6 to arbitrate.
pub fn cancel_pending_source_focus(mut sync: Signal<SourceSyncState>) {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        cancel_focus_guards_through(token);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInteractionOrigin {
    kind: &'static str,
    invocation: &'static str,
    launch_id: Option<Cow<'static, str>>,
    current_mode: Option<EditorMode>,
    removal_policy: &'static str,
}

impl SourceInteractionOrigin {
    pub const fn persistent_control(launch_id: &'static str) -> Self {
        Self {
            kind: "persistentControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMustRemain",
        }
    }

    pub const fn removable_menu_control(launch_id: &'static str) -> Self {
        Self {
            kind: "removableMenuControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    pub const fn start_control(launch_id: &'static str) -> Self {
        Self {
            kind: "startControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    /// A workspace-tree row opening a document (task 014 §3.1). The id is
    /// namespaced so it can never equal a fixed launch id, and the relative
    /// path keeps it unique among visible rows.
    pub fn tree_row(relative_path: &Path) -> Self {
        Self::document_link(format!("tree:{}", relative_path.display()))
    }

    /// A backlink opening its source document (task 014 §3.1). One file can
    /// link twice on one line, so the list position is part of the id.
    pub fn backlink(position: usize, source_path: &Path, line_number: usize) -> Self {
        Self::document_link(format!(
            "backlink:{position}:{}:{line_number}",
            source_path.display()
        ))
    }

    /// A search result opening its document (task 016). Like backlinks, one
    /// file can match twice on a line, so the list position is in the id.
    pub fn search_result(position: usize, relative_path: &Path, line_number: usize) -> Self {
        Self::document_link(format!(
            "search:{position}:{}:{line_number}",
            relative_path.display()
        ))
    }

    fn document_link(launch_id: String) -> Self {
        Self {
            kind: "documentLink",
            invocation: "pointer",
            launch_id: Some(Cow::Owned(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    pub fn launch_id(&self) -> Option<&str> {
        self.launch_id.as_deref()
    }

    fn shortcut(current_mode: EditorMode) -> Self {
        Self {
            kind: match current_mode {
                EditorMode::Text | EditorMode::Split => "replacedSourceSurface",
                EditorMode::Preview | EditorMode::Form => "replacedGeneralSurface",
            },
            invocation: "shortcut",
            launch_id: None,
            current_mode: Some(current_mode),
            removal_policy: "launchMayBeRemoved",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArmRequest<'a> {
    token: u64,
    fingerprint: &'a str,
    origin_kind: &'a str,
    invocation: &'a str,
    launch_id: Option<&'a str>,
    current_mode: Option<&'a str>,
    removal_policy: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuardArmed {
    token: u64,
    armed: bool,
    reason: Option<String>,
}

pub fn submit_source_interaction(
    sync: Signal<SourceSyncState>,
    _state: Signal<AppState>,
    mode: Signal<EditorMode>,
    _toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    finalize_launch_ui: impl FnOnce() + 'static,
) {
    submit_interaction(sync, mode, command, origin, finalize_launch_ui);
}

pub fn submit_source_shortcut_interaction(
    sync: Signal<SourceSyncState>,
    _state: Signal<AppState>,
    mode: Signal<EditorMode>,
    _toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) {
    let current_mode = *mode.read();
    submit_interaction(
        sync,
        mode,
        command,
        SourceInteractionOrigin::shortcut(current_mode),
        || {},
    );
}

/// Decides, synchronously, at the moment of submission (re-review,
/// 2026-10-02 §2: the first queueing pass moved this decision into the
/// queue, which broke two things -- see below -- so it moves back here).
/// [`claims_focus`] must predict this decision exactly (its own doc
/// comment), which only holds if both read the same `sync`/`mode` state
/// at the same instant; deferring the decision into the queue let
/// something queued ahead of it change that state first (§2.1).
/// `submit_source_command`'s own `cancel_focus_interactions` also runs
/// synchronously, so a direct command submitted just after a click still
/// cancels that click's claim -- which only holds if the claim was
/// already allocated by the time the direct command's own cancellation
/// runs, not allocated later from inside the queue (§2.2).
///
/// Only the async remainder -- arm, claim, finalize, submit -- is
/// queued, in [`run_interaction`].
fn submit_interaction(
    mut sync: Signal<SourceSyncState>,
    mode: Signal<EditorMode>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    finalize_launch_ui: impl FnOnce() + 'static,
) {
    let current_mode = *mode.read();
    // Calls `claims_focus` itself, rather than repeating its formula here,
    // so the two can never disagree about whether this moment's `command`
    // claims focus -- the exact property the re-review's §2.1 needs.
    if !claims_focus(&command, current_mode, &sync.read()) {
        finalize_launch_ui();
        enqueue_without_focus_claim(sync, command);
        return;
    }
    let target =
        focus_target(&command, current_mode).expect("claims_focus already confirmed a target");
    let fingerprint = format!(
        "{}:{}:{}:{}",
        origin.kind,
        origin.launch_id().unwrap_or("surface"),
        mode_name(current_mode),
        editor_name(target),
    );
    let Some((token, superseded)) = sync
        .write()
        .allocate_focus_interaction(target, fingerprint.clone())
    else {
        cancel_source_focus(sync);
        finalize_launch_ui();
        enqueue_without_focus_claim(sync, command);
        return;
    };
    if let Some(old_token) = superseded {
        cancel_focus_guards_through(old_token);
    }
    crate::bridge::trace("source.focus.interaction.allocate", token);
    super::queue::enqueue(super::queue::Submission::WithFocusClaim {
        command,
        origin,
        token,
        fingerprint,
        finalize_launch_ui: Box::new(finalize_launch_ui),
    });
}

/// `submit_source_command`'s own synchronous cleanup -- cancelling
/// whatever focus interaction is currently pending -- shared with
/// `submit_interaction`'s two "does not claim focus after all" branches,
/// so they do exactly what calling the public `submit_source_command`
/// did before this module enqueued instead of calling it directly.
fn enqueue_without_focus_claim(mut sync: Signal<SourceSyncState>, command: SourceCommand) {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        cancel_focus_guards_through(token);
    }
    super::queue::enqueue(super::queue::Submission::WithoutFocusClaim { command });
}

/// What [`super::queue::SourceCommandQueue`] does for a
/// [`super::queue::Submission::WithFocusClaim`], in the order its
/// `enqueue` call landed: the arm sequence, `submit_interaction`'s own
/// old body, unchanged in its own logic, just no longer `spawn`ed per
/// call -- it now runs as part of the single consumer's own `.await`
/// chain, so the *next* submission in the queue cannot reach
/// `submit_with_focus` before this one does. `token`/`fingerprint` were
/// already decided, synchronously, by `submit_interaction` itself.
pub(super) async fn run_interaction(
    signals: super::AppSignals,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    token: u64,
    fingerprint: String,
    finalize_launch_ui: Box<dyn FnOnce()>,
) {
    let mut sync = signals.sync;

    // No `spawn` here -- already running inside the single consumer task
    // (task 048 §2.3), so the next queued submission waits for this
    // `.await` chain to finish, exactly as it waits for everything above.
    //
    // Task 039 §2.1, and the review's §3.1: the trace says *which* way the
    // arm failed to resolve, since only an elapsed `ARM_TIMEOUT` is a
    // genuine timeout -- an early `None` from the old collapsed
    // `.ok().flatten()` here read identically whether the page never
    // answered in time or its query was dropped out from under it (task
    // 029/036's finding). The decision itself lives in `arm_resolution`,
    // not here, so a regression collapsing it back together fails that
    // function's own test.
    let outcome = timed_arm_outcome(
        tokio::time::timeout(ARM_TIMEOUT, arm_focus_guard(token, &fingerprint, &origin)).await,
    );
    let (ack, trace) = arm_resolution(token, outcome);
    if let Some((event, detail)) = trace {
        crate::bridge::trace(event, detail);
    }
    let armed = ack
        .as_ref()
        .is_some_and(|ack| ack.token == token && ack.armed);
    if let Some(ack) = ack.as_ref()
        && !armed
    {
        crate::bridge::trace(
            "source.focus.guard.rejected",
            ack.reason.as_deref().unwrap_or("invalidAcknowledgement"),
        );
    }
    let resolution = if armed {
        FocusResolution::Armed
    } else {
        FocusResolution::ProceedWithoutFocus
    };
    if sync.write().claim_focus_interaction(token, resolution) == FocusClaim::Stale {
        // Review §2.3: a claim that went stale while queued (superseded
        // by a newer one, or cancelled by a later direct command) must
        // not drop its command silently -- it still runs, just without
        // claiming focus. Chosen uniformly, for every cause of
        // staleness: nothing here can distinguish *which* cause this
        // was, and running without focus is always safe, so there is
        // nothing left to report through the discard path either -- that
        // alternative is for a cause where dropping would be correct,
        // and this task never drops.
        cancel_focus_guards_through(token);
        finalize_launch_ui();
        super::process_direct_submission(signals, command).await;
        return;
    }
    if armed {
        crate::bridge::trace("source.focus.guard.armed", token);
    } else {
        cancel_focus_guards_through(token);
    }
    finalize_launch_ui();
    let outcome =
        super::submit_source_command_preserving_focus(signals, command, Some(token)).await;
    crate::bridge::trace("source.focus.command.queued", format!("{outcome:?}"));
    if matches!(
        outcome,
        SubmitOutcome::NoOp | SubmitOutcome::QueueFull | SubmitOutcome::Unavailable
    ) {
        sync.write().cancel_focus_token(token);
        cancel_focus_guards_through(token);
    }
}

/// Whether `command` claims editor focus in `current_mode` -- the test that
/// separates a handoff activation from explicit dismissal (RFC-042 §6.2 rule
/// 3, as clarified for task 016).
///
/// It must predict `submit_interaction` exactly, so it mirrors **both** of
/// that function's early-return conditions: no target, or a `SwitchMode` into
/// the source editor the controller is already heading for
/// (`SourceSyncState::is_same_source_mode`, task 022) -- not `current_mode`
/// alone, which is only the UI's mode signal and can disagree with it for as
/// long as a switch is in flight. Otherwise a handoff would release authority
/// for a claim that is never made, and nothing would restore focus.
pub(super) fn claims_focus(
    command: &SourceCommand,
    current_mode: EditorMode,
    sync: &SourceSyncState,
) -> bool {
    focus_target(command, current_mode).is_some() && !sync.is_same_source_mode(command)
}

/// The editor a command claims focus for. `OpenDocument` carries no mode and
/// does not switch one, so its claim follows the mode the app is already in
/// (task 014 §2); `NewUntitled` forces Text and `SwitchMode` names its mode.
fn focus_target(command: &SourceCommand, current_mode: EditorMode) -> Option<SourceEditorId> {
    let mode = match command {
        SourceCommand::NewUntitled => EditorMode::Text,
        SourceCommand::SwitchMode(target) => *target,
        SourceCommand::OpenDocument(_) => current_mode,
        _ => return None,
    };
    match mode {
        EditorMode::Text => Some(SourceEditorId::Text),
        EditorMode::Split => Some(SourceEditorId::Split),
        EditorMode::Preview | EditorMode::Form => None,
    }
}

/// Why [`arm_focus_guard`] did not resolve to a decided acknowledgement
/// (task 039 §2.1). Each variant gets its own trace event at the call site,
/// carrying the detail here -- only an elapsed `ARM_TIMEOUT` (the caller's
/// own `tokio::time::timeout`, not a variant of this type) is a genuine
/// timeout.
#[derive(Debug)]
enum ArmFailure {
    /// `eval.recv` itself failed, with the `EvalError`'s own text. This is
    /// the signature of the page's query being dropped before Rust read it
    /// (task 039 §1): a `recv` failure this early, well inside `ARM_TIMEOUT`,
    /// is not evidence the page was slow.
    Unanswered(String),
    /// The page answered, but its payload did not decode as `GuardArmed`;
    /// carries the undecodable payload itself.
    Undecodable(String),
    /// `ArmRequest` itself did not serialize; carries the encoder's error.
    /// No eval is ever started in this case.
    Unencodable(String),
}

/// The trace event name and detail for one [`ArmFailure`] (task 039 §2.1).
/// Pure, and the only place that names these three events, so it is unit
/// tested directly: every variant gets its own name, and none of them is
/// `source.focus.guard.timeout` -- that name is reserved for an actually
/// elapsed `ARM_TIMEOUT`, named at the call site instead, not here.
fn arm_failure_trace(failure: &ArmFailure) -> (&'static str, &str) {
    match failure {
        ArmFailure::Unanswered(detail) => ("source.focus.guard.unanswered", detail.as_str()),
        ArmFailure::Undecodable(detail) => ("source.focus.guard.undecodable", detail.as_str()),
        ArmFailure::Unencodable(detail) => ("source.focus.guard.unencodable", detail.as_str()),
    }
}

/// What `tokio::time::timeout(ARM_TIMEOUT, arm_focus_guard(..))` resolved to,
/// collapsing tokio's own `Elapsed` (opaque, with no public constructor, so a
/// type this module defines itself can be built directly in tests) into one
/// variant.
enum ArmOutcome {
    Armed(GuardArmed),
    Failed(ArmFailure),
    TimedOut,
}

/// Collapses `tokio::time::timeout(ARM_TIMEOUT, arm_focus_guard(..))`'s result
/// into an [`ArmOutcome`]. Pure, and generic over the elapsed-error type so a
/// test can pass `Err(())` in place of tokio's own `Elapsed` (which has no
/// public constructor) -- task 040 §2.2. Before this was pulled out, the call
/// site itself was the one untested place this three-way decision was made:
/// the review's M3 mutation (`Ok(Err(_failure)) => ArmOutcome::TimedOut`),
/// reproduced here, passed every test that existed (`arm_resolution` only
/// covers what it is handed, not how the handed value was produced).
fn timed_arm_outcome<E>(result: Result<Result<GuardArmed, ArmFailure>, E>) -> ArmOutcome {
    match result {
        Ok(Ok(ack)) => ArmOutcome::Armed(ack),
        Ok(Err(failure)) => ArmOutcome::Failed(failure),
        Err(_elapsed) => ArmOutcome::TimedOut,
    }
}

/// Review (2026-10-01), required before merging: M3 showed that mistracing a
/// failed arm as `source.focus.guard.timeout` passed every test that existed,
/// because the call site's own three-way branch -- not just
/// [`arm_failure_trace`]'s narrower mapping -- was where that mistake could
/// happen, and nothing tested *that* decision directly. This is now the one
/// place that makes it, pure, so a regression there fails a test by name.
/// Only [`ArmOutcome::TimedOut`] may ever choose `source.focus.guard.timeout`.
fn arm_resolution(
    token: u64,
    outcome: ArmOutcome,
) -> (Option<GuardArmed>, Option<(&'static str, String)>) {
    match outcome {
        ArmOutcome::Armed(ack) => (Some(ack), None),
        ArmOutcome::Failed(failure) => {
            let (event, detail) = arm_failure_trace(&failure);
            (None, Some((event, detail.to_string())))
        }
        ArmOutcome::TimedOut => (
            None,
            Some(("source.focus.guard.timeout", token.to_string())),
        ),
    }
}

async fn arm_focus_guard(
    token: u64,
    fingerprint: &str,
    origin: &SourceInteractionOrigin,
) -> Result<GuardArmed, ArmFailure> {
    let request = ArmRequest {
        token,
        fingerprint,
        origin_kind: origin.kind,
        invocation: origin.invocation,
        launch_id: origin.launch_id(),
        current_mode: origin.current_mode.map(mode_name),
        removal_policy: origin.removal_policy,
    };
    let payload = serde_json::to_string(&request)
        .map_err(|error| ArmFailure::Unencodable(error.to_string()))?;
    let mut eval = document::eval(&arm_focus_guard_js(&payload));
    let payload = eval
        .recv::<String>()
        .await
        .map_err(|error| ArmFailure::Unanswered(error.to_string()))?;
    // Task 039 §2.2: release the page's pinned query now that its
    // acknowledgement has been read, whether or not it goes on to decode
    // below -- the page cannot know a decode failure happened on this side,
    // and its own bound (`GUARD_RELEASE_TIMEOUT_MS`) exists exactly so a
    // missed release here is not fatal to it either way.
    let _ = eval.send(true);
    decode_guard_acknowledgement(&payload).ok_or(ArmFailure::Undecodable(payload))
}

/// The script that arms a focus guard. `payload` is the serialized
/// `ArmRequest`; it reaches the page as a string literal the script
/// `JSON.parse`s (task 031), not as JavaScript source.
///
/// Task 039 §2.2: the evaluated function **returns** its promise (the
/// leading `return`), and that promise does not settle until Rust's release
/// arrives, or its own bound elapses. Without the `return`, as this script
/// read before task 039, the IIFE's promise resolves on its own as soon as
/// it is created, right after `dioxus.send(ack)` -- Dioxus's wrapper then
/// calls `dioxus.close()`, which only nulls the JS message queue, and its
/// `FinalizationRegistry` can post the slab-entry drop that frees the
/// evaluator, and the unread acknowledgement with it, before the spawned
/// Rust task ever polls `recv` (task 039 §1; the same hazard
/// `webview_smoke/transport.rs:11-16` documents for the smoke transport).
/// Re-audit `native_eval.ts`, `query.rs`, `document.rs` and
/// `dioxus-document` `eval.rs` before updating Dioxus.
fn arm_focus_guard_js(payload: &str) -> String {
    let payload = crate::bridge::js_string_literal(payload);
    format!(
        r#"
        {FOCUS_GUARD_BOOTSTRAP}
        return (async () => {{
            const request = JSON.parse({payload});
            const guards = window.__bkFocusGuards;
            const ack = (!guards
                || guards.protocolVersion !== {FOCUS_GUARD_PROTOCOL_VERSION}
                || typeof guards.arm !== "function")
                ? {{ token: request.token, armed: false, reason: "incompatibleRegistry" }}
                : guards.arm(request);
            dioxus.send(JSON.stringify(ack));
            // Stay alive until Rust releases this query, bounded so a
            // dropped ARM_TIMEOUT race (Rust stopped waiting and never
            // sends a release) cannot leave this promise pending forever.
            await Promise.race([
                dioxus.recv(),
                new Promise((resolve) => setTimeout(resolve, {GUARD_RELEASE_TIMEOUT_MS})),
            ]);
            return null;
        }})();
        "#,
    )
}

fn decode_guard_acknowledgement(payload: &str) -> Option<GuardArmed> {
    serde_json::from_str(payload).ok()
}

pub(crate) fn cancel_focus_guards_through(token: u64) {
    document::eval(&format!(
        r#"
        {FOCUS_GUARD_BOOTSTRAP}
        if (window.__bkFocusGuards?.protocolVersion === {FOCUS_GUARD_PROTOCOL_VERSION}
            && typeof window.__bkFocusGuards.cancelThrough === "function") {{
            window.__bkFocusGuards.cancelThrough({token});
        }}
        "#,
    ));
}

pub(crate) fn consume_focus_guard(token: u64, identity: EditorIdentity, fingerprint: &str) {
    let identity = serde_json::to_string(&identity).expect("editor identity serializes");
    document::eval(&consume_focus_guard_js(token, &identity, fingerprint));
}

/// The script that consumes a focus guard: the identity (serialized JSON)
/// and the fingerprint reach the page as string literals (task 031).
fn consume_focus_guard_js(token: u64, identity: &str, fingerprint: &str) -> String {
    let identity = crate::bridge::js_string_literal(identity);
    let fingerprint = crate::bridge::js_string_literal(fingerprint);
    format!(
        r#"
        if (window.__bk && typeof window.__bk.consumeFocusGuard === "function") {{
            window.__bk.consumeFocusGuard({{ token: {token}, identity: JSON.parse({identity}), fingerprint: {fingerprint} }});
        }}
        "#,
    )
}

fn mode_name(mode: EditorMode) -> &'static str {
    match mode {
        EditorMode::Text => "text",
        EditorMode::Preview => "preview",
        EditorMode::Form => "form",
        EditorMode::Split => "split",
    }
}

fn editor_name(editor: SourceEditorId) -> &'static str {
    match editor {
        SourceEditorId::Text => "text",
        SourceEditorId::Split => "split",
    }
}

#[cfg(test)]
mod tests;
