//! RFC-044 §2's gating spike: does a synthetic `KeyboardEvent` dispatched
//! by a driver reach a Dioxus `onkeydown` handler, through Dioxus's own
//! event plumbing, in a real WebView? Everything in RFC-044 §8 depends on
//! the answer, and nobody has verified it.
//!
//! Deliberately minimal and separate from the second run's architecture
//! (handoff §2: "before the transport work, before the phase machine,
//! before any contract") -- this proves one fact using the smallest
//! reachable observable consequence: dispatch one `ArrowDown` at the
//! workspace tree's roving-tabindex row, and check whether
//! `document.activeElement` moves to the next row, which only happens if
//! `TreeRowItem`'s `onkeydown` handler ran `shell_focus::focus_tree_row`
//! (`crates/bekoedit-app/src/components/explorer/tree_row.rs`).
//!
//! Reuses RFC-043's launch-time reopen (the reason that RFC was built) to
//! reach the shell without a file-picker dialog: seeds a two-file
//! workspace, a recents entry pointing to it, and
//! `reopen_last_workspace: true`, exactly as a real user's profile would
//! be, then lets `App()`'s existing launch path do the rest.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use bekoedit_fs::RecentWorkspaces;

use crate::persistence::AppPersistence;
use crate::settings::AppSettings;

use super::SmokeProfile;

const POLL_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

const PASSED: u8 = 1;
const FAILED: u8 = 2;

#[derive(Debug, Default)]
pub struct KeySpikeTerminal {
    state: AtomicU8,
}

impl KeySpikeTerminal {
    fn mark(&self, passed: bool) {
        self.state
            .store(if passed { PASSED } else { FAILED }, Ordering::SeqCst);
    }

    pub(super) fn passed(&self) -> bool {
        self.state.load(Ordering::SeqCst) == PASSED
    }
}

pub(super) struct PreparedSpike {
    pub(super) root: PathBuf,
    pub(super) persistence: AppPersistence,
}

/// Creates an isolated profile, seeds a two-file workspace plus a recents
/// entry pointing to it, and enables `reopen_last_workspace` -- so the
/// normal launch path opens straight into the shell with a populated
/// tree, no dialog, no Start Screen.
pub(super) fn prepare(requested_root: &Path) -> Result<PreparedSpike, String> {
    let profile = SmokeProfile::create(requested_root)?;
    let paths = profile
        .persistence
        .isolated_paths()
        .expect("key spike persistence is always Isolated");

    let workspace = paths.root().join("workspace");
    std::fs::create_dir(&workspace)
        .map_err(|error| format!("cannot create key spike workspace: {error}"))?;
    std::fs::write(workspace.join("a.md"), "# a\n")
        .map_err(|error| format!("cannot seed key spike workspace file a.md: {error}"))?;
    std::fs::write(workspace.join("b.md"), "# b\n")
        .map_err(|error| format!("cannot seed key spike workspace file b.md: {error}"))?;

    let settings = AppSettings {
        reopen_last_workspace: true,
        ..Default::default()
    };
    profile
        .persistence
        .save_settings(&settings)
        .map_err(|error| format!("cannot seed key spike settings: {error}"))?;

    let mut recents = RecentWorkspaces::default();
    recents.record(workspace, "workspace".to_string(), 1);
    recents
        .save(&profile.persistence.recents_file())
        .map_err(|error| format!("cannot seed key spike recents: {error}"))?;

    Ok(PreparedSpike {
        root: profile.root,
        persistence: profile.persistence,
    })
}

#[component]
pub fn WebViewKeySpikeDriver() -> Element {
    let desktop: DesktopContext = consume_context();
    let terminal = super::launch_config()
        .key_spike
        .clone()
        .expect("key spike driver requires spike terminal state");
    use_future(move || {
        let terminal = terminal.clone();
        let desktop = desktop.clone();
        async move {
            println!(
                "bekoedit RFC-044 §2 spike: does a synthetic KeyboardEvent reach a Dioxus onkeydown handler?"
            );
            match run_spike().await {
                Ok(()) => {
                    println!(
                        "bekoedit RFC-044 §2 spike PASSED: a synthetic ArrowDown moved document.activeElement to the next tree row"
                    );
                    terminal.mark(true);
                }
                Err(error) => {
                    eprintln!("bekoedit RFC-044 §2 spike FAILED: {error}");
                    terminal.mark(false);
                }
            }
            desktop.close();
        }
    });
    rsx! {}
}

async fn run_spike() -> Result<(), String> {
    poll_until(
        "the workspace tree renders at least two rows",
        "document.querySelectorAll('[data-tree-row]').length >= 2",
    )
    .await?;

    let focused: bool = document::eval(
        r#"return (() => {
            const rows = document.querySelectorAll('[data-tree-row]');
            if (rows.length < 2) return false;
            rows[0].focus();
            return document.activeElement === rows[0];
        })();"#,
    )
    .join()
    .await
    .map_err(|error| format!("could not focus the first tree row: {error}"))?;
    if !focused {
        return Err(
            "focusing the first tree row directly did not work -- a setup failure, not the hypothesis under test"
                .into(),
        );
    }

    let dispatched: bool = document::eval(
        r#"return document.querySelectorAll('[data-tree-row]')[0].dispatchEvent(
            new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));"#,
    )
    .join()
    .await
    .map_err(|error| format!("could not dispatch the synthetic ArrowDown keydown: {error}"))?;
    if !dispatched {
        return Err("dispatchEvent for the synthetic ArrowDown keydown returned false".into());
    }

    poll_until(
        "document.activeElement moves to the second tree row after the synthetic ArrowDown \
         (this only happens if TreeRowItem's onkeydown handler ran and called \
         shell_focus::focus_tree_row)",
        "document.querySelectorAll('[data-tree-row]')[1] === document.activeElement",
    )
    .await
}

/// Polls `condition_js` (a JS expression, no trailing semicolon) with a
/// deadline. Never sleeps as the wait mechanism itself between polls
/// beyond the fixed, named interval -- the poll loop *is* the wait, and
/// every call site names what it is waiting for (RFC-044 §7/§10).
async fn poll_until(description: &str, condition_js: &str) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + POLL_TIMEOUT;
    loop {
        let ok: bool = document::eval(&format!("return {condition_js};"))
            .join()
            .await
            .map_err(|error| format!("eval failed while waiting for {description}: {error}"))?;
        if ok {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for: {description}"));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
