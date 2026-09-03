//! RFC-044 slice-1 §4/§5: the second WebView run -- shell behaviour
//! coverage, starting with §8 A's seven tree-navigation contracts.
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
//! The workspace fixture is shaped for §8 A's seven contracts specifically:
//! `sub/child.md` (an expandable directory with one child, for contracts 3
//! and 4), `a.md` and `z.md` (openable, for Home/End and Enter), and
//! `notes.txt` (not markdown, so not openable -- contract 6).
//!
//! Contract 1 (Tab reaches the tree at exactly one stop) is not its own
//! phase: per the governance review that corrected RFC-044 §8 A.1
//! (2026-09-03), a synthetic Tab cannot drive focus, since browsers
//! withhold default actions from untrusted, script-dispatched events.
//! Mechanism C instead asserts the roving-tabindex invariant live inside
//! `shell_behaviour_driver.js`, after each of contracts 2-5's own
//! app-intercepted nav keys.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_fs::RecentWorkspaces;

use crate::persistence::AppPersistence;
use crate::settings::AppSettings;

use super::SmokeProfile;
use super::transport::{
    self, CompletedProbe, DriverResult, MessageKind, PhaseKind, PhaseMessage, PinnedExchange,
    SMOKE_PROTOCOL_VERSION,
};

const MARKER: &str = "RFC044_SHELL_BEHAVIOUR_MARKER";
const EXPECTED_MILESTONES: [&str; 6] = [
    "down_up_moved",
    "expand_entered",
    "collapse_ascended",
    "home_end_reached",
    "non_openable_reachable",
    "enter_opened_editor_focused",
];
const PHASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

const SHELL_BEHAVIOUR_JS: &str = include_str!("shell_behaviour_driver.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShellBehaviourPhase {
    DownUp,
    ExpandEnter,
    CollapseAscend,
    HomeEnd,
    NonOpenable,
    EnterOpens,
}

impl ShellBehaviourPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::DownUp => "down_up",
            Self::ExpandEnter => "expand_enter",
            Self::CollapseAscend => "collapse_ascend",
            Self::HomeEnd => "home_end",
            Self::NonOpenable => "non_openable",
            Self::EnterOpens => "enter_opens",
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::DownUp => Some(Self::ExpandEnter),
            Self::ExpandEnter => Some(Self::CollapseAscend),
            Self::CollapseAscend => Some(Self::HomeEnd),
            Self::HomeEnd => Some(Self::NonOpenable),
            Self::NonOpenable => Some(Self::EnterOpens),
            Self::EnterOpens => None,
        }
    }

    /// The `milestone` a `Progress` report from this phase must carry --
    /// one-to-one with `EXPECTED_MILESTONES`.
    const fn expected_milestone(self) -> &'static str {
        match self {
            Self::DownUp => "down_up_moved",
            Self::ExpandEnter => "expand_entered",
            Self::CollapseAscend => "collapse_ascended",
            Self::HomeEnd => "home_end_reached",
            Self::NonOpenable => "non_openable_reachable",
            Self::EnterOpens => "enter_opened_editor_focused",
        }
    }
}

impl PhaseKind for ShellBehaviourPhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}

#[derive(Debug)]
struct ShellBehaviourMachine {
    current: ShellBehaviourPhase,
    last_applied_exchange_id: Option<u64>,
}

impl ShellBehaviourMachine {
    const fn new() -> Self {
        Self {
            current: ShellBehaviourPhase::DownUp,
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
                if self.current == ShellBehaviourPhase::EnterOpens {
                    return Err("enter_opens phase cannot return nonterminal progress".into());
                }
                let expected = self.current.expected_milestone();
                if message.milestone.as_deref() != Some(expected) || message.result.is_some() {
                    return Err("driver returned malformed phase progress".into());
                }
            }
            MessageKind::Terminal => {
                if self.current != ShellBehaviourPhase::EnterOpens {
                    return Err("only enter_opens can return a terminal result".into());
                }
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
    if result.stage != "enter_opens" || result.marker != MARKER {
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
}

impl ShellBehaviourTerminal {
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
    std::fs::write(sub.join("child.md"), "# child\n")
        .map_err(|error| format!("cannot seed shell-behaviour child.md: {error}"))?;
    std::fs::write(workspace.join("a.md"), "# a\n")
        .map_err(|error| format!("cannot seed shell-behaviour a.md: {error}"))?;
    std::fs::write(workspace.join("notes.txt"), "not markdown\n")
        .map_err(|error| format!("cannot seed shell-behaviour notes.txt: {error}"))?;
    std::fs::write(workspace.join("z.md"), "# z\n")
        .map_err(|error| format!("cannot seed shell-behaviour z.md: {error}"))?;

    let settings = AppSettings {
        reopen_last_workspace: true,
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

async fn run_shell_behaviour_sequence(
    terminal: &ShellBehaviourTerminal,
) -> Result<DriverResult, String> {
    let mut machine = ShellBehaviourMachine::new();
    let mut exchange_id = 1_u64;
    let mut release = None;
    loop {
        let completed = run_shell_behaviour_phase(machine.current(), exchange_id, release).await?;
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
        exchange_id = exchange_id
            .checked_add(1)
            .ok_or_else(|| "exchange id exhausted".to_string())?;
        tokio::time::sleep(PHASE_POLL_INTERVAL).await;
    }
}

#[component]
pub fn WebViewShellBehaviourDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let terminal = super::launch_config()
        .shell_behaviour
        .clone()
        .expect("shell behaviour driver requires terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            println!("bekoedit RFC-044 shell-behaviour run: tree navigation (§8 A)");
            match run_shell_behaviour_sequence(&terminal).await {
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
