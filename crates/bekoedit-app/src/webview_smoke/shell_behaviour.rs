//! RFC-044 slice-1 §4/§5: the second WebView run -- shell behaviour
//! coverage, starting with §8 A's tree-navigation contracts 1-6.
//!
//! A separate run from the RFC-041 lifecycle regression, per RFC-044 §5:
//! its own launch flag, its own driver JS (`shell_behaviour_driver.js`),
//! its own phase set and milestone list. It shares only the evaluator-pin
//! transport (`super::transport`) and the disposable `Isolated` profile
//! machinery (`SmokeProfile`) -- `--webview-smoke` and `driver.js` are
//! untouched by this module.
//!
//! Reuses RFC-043's launch-time reopen exactly as the §2 spike did: seeds
//! a workspace, a recents entry, and `reopen_last_workspace`, so the run
//! lands in the shell with a populated tree, no dialog, no Start Screen.
//! The workspace fixture is shaped for §8 A's contracts: `sub/child.md` (an
//! expandable directory with one child, for contracts 3 and 4), `a.md` and
//! `z.md` (openable, for Home/End and Enter), and `notes.txt` (not markdown,
//! so not openable -- contract 6).
//!
//! Contract 1 (Tab reaches the tree at exactly one stop) is not its own
//! phase: per the governance review that corrected RFC-044 §8 A.1
//! (2026-09-03), a synthetic Tab cannot drive focus, since browsers
//! withhold default actions from untrusted, script-dispatched events.
//! Mechanism C instead asserts the roving-tabindex invariant live inside
//! `shell_behaviour_driver.js`, after each of contracts 2-5's own
//! app-intercepted nav keys.
//!
//! Contract 7 ("Enter opens a document and the editor takes focus") is
//! `EnterOpens`. Slice 1 deferred it because `OpenDocument` did not claim
//! editor focus (`source_sync::focus::focus_target`); task 014 landed it
//! ahead of the fix, so it was proven able to fail by a red CI run rather
//! than by a mutation afterwards.
//!
//! Task 016's handoff-activation contracts follow it (RFC-042 §6.2 rule 3):
//! `SearchResultOpens` (a search result opens its document and the editor
//! takes focus, not the search trigger), then `NewFileFocuses` and the
//! terminal `TreeEnterAfterNewFile` -- App menu "New File" focuses the
//! editor, and a tree Enter afterwards still does, which only holds if the
//! menu released shell authority. Committed before the fix, like contract 7.
//!
//! `FormSearchRestores` (task 016 re-review §2) covers the mode users get by
//! default: in Form, a search result claims no editor focus, so it is not a
//! handoff -- explicit dismissal restores focus to the search trigger.
//!
//! Slice 2 appends a mouse-open phase for the app menu (task 017 §2: a mouse
//! open leaves focus on the trigger, alone and after a keyboard open and
//! close, so no keyboard entry intent is left behind), then RFC-044 §8 B's
//! menu-button contracts, three phases per overflow menu: the trigger and in-menu keys (contracts 1-5), Escape
//! restoring focus to the trigger (6), and focus leaving the wrap closing it
//! without restoring (7). Contract 7 moves focus with a script `focus()`
//! rather than a synthetic Tab, per the slice-2 handoff §4.
//!
//! Slice 3 stage 1 appends RFC-044 §8 C (mode tabs: arrows move focus only;
//! activating Text selects it and focuses the editor) and §8 D (focus
//! entering the editor closes an open menu and stays; that close released
//! shell authority, proven by a later claim succeeding). The phase table
//! lives in `phase.rs` (slice 3 handoff §4.4).
//!
//! Slice 3 stage 2 adds §8 E and F. Recovery only appears at launch, so
//! `prepare` seeds one snapshot and its two phases run first. Settings is
//! entered and closed through static ids. For F, `prepare` seeds a one-day
//! autosave debounce so the document stays dirty, and the sequence writes a
//! change to the open file on disk between F1 and F2 (§4.2).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_fs::{RecentWorkspaces, RecoverySnapshot, RecoveryStore, UserSettings};

use crate::persistence::AppPersistence;
use crate::settings::AppSettings;
use crate::source_sync::SourceSyncState;

use super::SmokeProfile;
use super::transport::{
    self, CompletedProbe, DriverResult, MessageKind, PhaseMessage, PinnedExchange,
    SMOKE_PROTOCOL_VERSION, reject_with_reason,
};

mod phase;
use phase::{EXPECTED_MILESTONES, ShellBehaviourPhase, TERMINAL_STAGE};

const MARKER: &str = "RFC044_SHELL_BEHAVIOUR_MARKER";
const PHASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

const SHELL_BEHAVIOUR_JS: &str = include_str!("shell_behaviour_driver.js");

#[derive(Debug)]
struct ShellBehaviourMachine {
    current: ShellBehaviourPhase,
    last_applied_exchange_id: Option<u64>,
}

impl ShellBehaviourMachine {
    const fn new() -> Self {
        Self {
            current: ShellBehaviourPhase::RecoveryEntry,
            last_applied_exchange_id: None,
        }
    }

    const fn current(&self) -> ShellBehaviourPhase {
        self.current
    }

    const fn for_phase(current: ShellBehaviourPhase) -> Self {
        Self {
            current,
            last_applied_exchange_id: None,
        }
    }

    fn validate(
        &self,
        message: &PhaseMessage,
        exchange_id: u64,
        release: Option<PinnedExchange<ShellBehaviourPhase>>,
    ) -> Result<(), String> {
        // Task 025 §2.3: a structural rejection carries the driver's own
        // reason along, when the message is a terminal failure that has
        // one (e.g. shell_behaviour_driver.js's failEarly, which always
        // reports releasedExchangeId: null since it cannot know what Rust
        // actually expected released -- trusted_click.rs's own validate()
        // is the finding this generalises).
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
                // A terminal report can come from any phase, not only the
                // last one -- the driver's try/catch turns a thrown error
                // into a terminal failure at whichever phase raised it,
                // exactly as driver.js's own three phases each can.
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

fn validate_shell_behaviour_result(result: &DriverResult) -> Result<(), String> {
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
        return Err("an error toast appeared during the shell-behaviour sequence".into());
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
pub struct ShellBehaviourTerminal {
    state: AtomicU8,
    /// The document F's Rust-side write targets, handed over from `prepare`
    /// rather than rediscovered at run time (slice 3 handoff §4.2).
    conflict_file: Option<PathBuf>,
}

impl ShellBehaviourTerminal {
    pub(super) fn with_conflict_file(conflict_file: PathBuf) -> Self {
        Self {
            state: AtomicU8::new(0),
            conflict_file: Some(conflict_file),
        }
    }

    fn accept(&self, result: &DriverResult) -> Result<(), String> {
        validate_shell_behaviour_result(result)?;
        self.state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "shell-behaviour terminal result was already recorded".to_string())?;
        Ok(())
    }

    pub(super) fn succeeded(&self) -> bool {
        self.state.load(Ordering::SeqCst) == 1
    }
}

pub(super) struct PreparedShellBehaviour {
    pub(super) root: PathBuf,
    pub(super) persistence: AppPersistence,
    pub(super) conflict_file: PathBuf,
}

/// Slice 3 §4.3: autosave must not clean F1's edit before the write. One
/// day, not `u64::MAX` -- `note_edit` adds the debounce to the current time.
pub(super) const CONFLICT_AUTOSAVE_DEBOUNCE_MS: u64 = 86_400_000;

/// Slice 3 §4.2: the Rust-side write that makes F2's conflict happens after
/// exactly this phase reports progress, and before F2 is requested.
pub(super) const fn writes_conflict_after(phase: ShellBehaviourPhase) -> bool {
    matches!(phase, ShellBehaviourPhase::ConflictDirtied)
}

/// Changes `path` on disk to content of a different length. Detection is
/// length plus content hash, not mtime (`bekoedit-fs/src/atomic.rs`).
pub(super) fn write_conflicting_change(path: &Path) -> Result<(), String> {
    let current = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read the conflict file {}: {error}", path.display()))?;
    let changed = format!("{current}\nChanged on disk by the RFC-044 harness (§8 F).\n");
    std::fs::write(path, changed).map_err(|error| {
        format!(
            "cannot write the conflict change to {}: {error}",
            path.display()
        )
    })
}

/// Creates an isolated profile and seeds a workspace shaped for §8 A's
/// seven contracts: a directory with one child (expand/enter, collapse/
/// ascend), two openable files bracketing a non-openable one (Home/End,
/// the non-openable-row contract, Enter-opens) -- plus the recents entry
/// and `reopen_last_workspace` setting RFC-043 needs to reach the shell
/// with no dialog.
pub(super) fn prepare(requested_root: &Path) -> Result<PreparedShellBehaviour, String> {
    let profile = SmokeProfile::create(requested_root)?;
    let paths = profile
        .persistence
        .isolated_paths()
        .expect("shell behaviour persistence is always Isolated");

    let workspace = paths.root().join("workspace");
    std::fs::create_dir(&workspace)
        .map_err(|error| format!("cannot create shell-behaviour workspace: {error}"))?;
    let sub = workspace.join("sub");
    std::fs::create_dir(&sub).map_err(|error| {
        format!("cannot create shell-behaviour workspace subdirectory: {error}")
    })?;
    let conflict_file = sub.join("child.md");
    std::fs::write(&conflict_file, "# child\n")
        .map_err(|error| format!("cannot seed shell-behaviour child.md: {error}"))?;
    std::fs::write(workspace.join("a.md"), "# a\n")
        .map_err(|error| format!("cannot seed shell-behaviour a.md: {error}"))?;
    // Slice 3 §4.1: one recovery snapshot, through the same API AppState
    // uses, so the run launches into the Recovery screen. Its text differs
    // from the file; only Skip all is ever clicked, so it is never restored.
    RecoveryStore::at(paths.recovery_dir().to_path_buf())
        .save(&RecoverySnapshot {
            original_path: workspace.join("a.md"),
            text: "# a (recovered by the RFC-044 harness)\n".into(),
            revision: 1,
            created_at_secs: 1,
        })
        .map_err(|error| format!("cannot seed shell-behaviour recovery snapshot: {error}"))?;
    std::fs::write(workspace.join("notes.txt"), "not markdown\n")
        .map_err(|error| format!("cannot seed shell-behaviour notes.txt: {error}"))?;
    std::fs::write(workspace.join("z.md"), "# z\n")
        .map_err(|error| format!("cannot seed shell-behaviour z.md: {error}"))?;

    let settings = AppSettings {
        reopen_last_workspace: true,
        // AppSettings::default() carries default_mode: EditorMode::Form
        // (settings.rs), and mode_sig's own initial value is
        // settings.default_mode (app.rs). OpenDocument never forces a mode
        // switch -- only NewUntitled does (source_sync/commands.rs) -- so
        // without this, contract 7's Enter never renders TextMode at all:
        // no editor host, no view, nothing to become ready. Not an app
        // defect -- opening a document into whatever mode you last used is
        // the intended behaviour -- just something this fixture must set
        // explicitly since RFC-044 slice-1 §5's contract needs Text mode.
        default_mode: bekoedit_ui_contract::EditorMode::Text,
        core: UserSettings {
            autosave_debounce_ms: CONFLICT_AUTOSAVE_DEBOUNCE_MS,
            ..Default::default()
        },
        ..Default::default()
    };
    profile
        .persistence
        .save_settings(&settings)
        .map_err(|error| format!("cannot seed shell-behaviour settings: {error}"))?;

    let mut recents = RecentWorkspaces::default();
    recents.record(workspace, "workspace".to_string(), 1);
    recents
        .save(&profile.persistence.recents_file())
        .map_err(|error| format!("cannot seed shell-behaviour recents: {error}"))?;

    Ok(PreparedShellBehaviour {
        root: profile.root,
        persistence: profile.persistence,
        conflict_file,
    })
}

/// Adapts the shared transport to this run's own phase semantics --
/// `ShellBehaviourPhase` and `ShellBehaviourMachine`'s validation -- the
/// same shape as `webview_smoke.rs`'s own `run_driver_phase` adapter for
/// RFC-041.
async fn run_shell_behaviour_phase(
    phase: ShellBehaviourPhase,
    exchange_id: u64,
    release: Option<PinnedExchange<ShellBehaviourPhase>>,
) -> Result<CompletedProbe<ShellBehaviourPhase>, String> {
    transport::run_driver_phase(SHELL_BEHAVIOUR_JS, phase, exchange_id, release, |message| {
        ShellBehaviourMachine::for_phase(phase).validate(message, exchange_id, release)
    })
    .await
}

/// Task 021: how the settle gate waits. `deadline` outlasts every deadline the
/// controller itself enforces (`source_sync::lifecycle`, at most 5 s for a
/// mount), so a state that is still busy at the end of it is not a slow
/// transition but a controller that is stuck.
#[derive(Debug, Clone, Copy)]
struct SettleGate {
    deadline: Duration,
    poll: Duration,
}

const SETTLE_GATE: SettleGate = SettleGate {
    deadline: Duration::from_secs(10),
    poll: Duration::from_millis(20),
};

/// Task 021: no exchange is requested while the source controller would answer
/// `Busy` to a command (`SourceSyncState::busy_lifecycle_state`).
///
/// D2 clicks Preview and then Text. Preview renders at once, but the Text
/// editor's teardown finishes only when the page's `destroyed` event reaches
/// Rust, and no DOM observable reports that (task 021's finding). A Text click
/// inside that window is dropped as `Busy`, so D2 timed out naming held
/// authority. The gate is here, on the Rust side, and applies to every phase,
/// so nothing acts inside a transition. No phase relies on doing so: each one
/// either starts from a settled shell or waits for its own async work.
///
/// A controller that never settles fails the run, naming the phase and the
/// state, instead of hanging or falling through to a wrong-cause timeout.
async fn wait_until_settled(
    phase: ShellBehaviourPhase,
    gate: SettleGate,
    mut busy_state: impl FnMut() -> Option<String>,
) -> Result<(), String> {
    let started = tokio::time::Instant::now();
    loop {
        let Some(state) = busy_state() else {
            return Ok(());
        };
        if started.elapsed() >= gate.deadline {
            return Err(format!(
                "the source controller did not settle before the {} exchange: \
                 still {state} after {:?}",
                phase.as_str(),
                gate.deadline
            ));
        }
        tokio::time::sleep(gate.poll).await;
    }
}

async fn run_shell_behaviour_sequence<RunPhase, PhaseFuture>(
    terminal: &ShellBehaviourTerminal,
    gate: SettleGate,
    mut busy_state: impl FnMut() -> Option<String>,
    mut run_phase: RunPhase,
) -> Result<DriverResult, String>
where
    RunPhase:
        FnMut(ShellBehaviourPhase, u64, Option<PinnedExchange<ShellBehaviourPhase>>) -> PhaseFuture,
    PhaseFuture: Future<Output = Result<CompletedProbe<ShellBehaviourPhase>, String>>,
{
    let mut machine = ShellBehaviourMachine::new();
    let mut exchange_id = 1_u64;
    let mut release = None;
    loop {
        let phase = machine.current();
        wait_until_settled(phase, gate, &mut busy_state).await?;
        let completed = run_phase(phase, exchange_id, release).await?;
        machine.validate(&completed.message, exchange_id, release)?;
        transport::validate_completion(
            &completed.completion,
            exchange_id,
            machine.current().as_str(),
            completed.message.kind,
        )?;
        machine.apply_completed(exchange_id, &completed.message)?;
        release = Some(completed.pin);
        if let Some(result) = completed.message.result {
            terminal.accept(&result)?;
            return Ok(result);
        }
        // §8 F (slice 3 §4.2): between F1's progress and F2's request.
        if completed.message.kind == MessageKind::Progress && writes_conflict_after(phase) {
            let path = terminal
                .conflict_file
                .as_deref()
                .ok_or_else(|| "no conflict file was prepared for §8 F".to_string())?;
            write_conflicting_change(path)?;
        }
        exchange_id = exchange_id
            .checked_add(1)
            .ok_or_else(|| "exchange id exhausted".to_string())?;
        tokio::time::sleep(PHASE_POLL_INTERVAL).await;
    }
}

#[component]
pub fn WebViewShellBehaviourDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let sync = use_context::<Signal<SourceSyncState>>();
    let terminal = super::launch_config()
        .shell_behaviour
        .clone()
        .expect("shell behaviour driver requires terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            println!("bekoedit RFC-044 shell-behaviour run: tree navigation (§8 A)");
            // A peek, not a read: the gate must not subscribe this component
            // to the controller. A borrow held elsewhere counts as not settled.
            let busy_state = move || match sync.try_peek() {
                Ok(state) => state.busy_lifecycle_state().map(str::to_string),
                Err(_) => Some("borrowed elsewhere".to_string()),
            };
            match run_shell_behaviour_sequence(
                &terminal,
                SETTLE_GATE,
                busy_state,
                run_shell_behaviour_phase,
            )
            .await
            {
                Ok(result) => {
                    for milestone in &result.milestones {
                        println!("  ✓ {milestone}");
                    }
                    println!("bekoedit RFC-044 shell-behaviour run PASSED");
                }
                Err(error) => eprintln!("bekoedit RFC-044 shell-behaviour run FAILED: {error}"),
            }
            desktop.close();
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests;
