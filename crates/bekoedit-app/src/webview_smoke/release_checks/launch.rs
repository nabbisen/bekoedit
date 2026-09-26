//! The three launch-time scenarios (#2, #3, #4): observe the app from its
//! first render, then judge what was seen with a pure function.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;

use bekoedit_core::AppState;

use crate::components::toast::{Toast, ToastKind};
use crate::i18n::{Lang, tr};

use super::dom::{self, DomSnapshot};
use super::{Expectation, ReleaseChecksTerminal, ReleaseScenario};

const DEADLINE: Duration = Duration::from_secs(15);
/// How long a scenario keeps watching after it looks right, so a toast, or a
/// late mount, that would contradict it has time to appear.
const WATCH_AFTER_READY: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_millis(40);

#[derive(Debug, Default)]
pub(super) struct Observation {
    pub start_screen_mounts: usize,
    pub dom: Option<DomSnapshot>,
    pub dom_error: Option<String>,
    pub workspaces: Vec<PathBuf>,
    pub session_seen: bool,
    pub toasts: BTreeMap<u64, (ToastKind, String)>,
}

/// Whether the scenario's own success condition is on screen yet.
fn looks_ready(scenario: ReleaseScenario, observation: &Observation) -> bool {
    let Some(dom) = &observation.dom else {
        return false;
    };
    match scenario {
        ReleaseScenario::ReopenUsable => dom.tree_rows >= 2,
        _ => dom.start_screen,
    }
}

fn same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Judges an observation. Every failure names the scenario, the check and what
/// was seen. Pure, so each rule is unit tested and mutation-proven.
pub(super) fn judge(
    scenario: ReleaseScenario,
    expectation: &Expectation,
    lang: Lang,
    observation: &Observation,
) -> Result<Vec<String>, String> {
    let name = scenario.name();
    let fail = |check: &str, saw: String| Err(format!("{name}: {check}: {saw}"));
    let dom = observation.dom.clone().unwrap_or_default();
    let mut passed = Vec::new();

    let toast_texts: Vec<String> = observation
        .toasts
        .values()
        .map(|(kind, message)| format!("{kind:?}: {message}"))
        .collect();
    let opened = |root: &PathBuf| root.display().to_string();

    match scenario {
        ReleaseScenario::ReopenUsable => {
            if observation.start_screen_mounts != 0 {
                return fail(
                    "the Start Screen never mounted",
                    format!(
                        "the Start Screen mounted {} time(s); RFC-043 decides the workspace \
                         before the first render",
                        observation.start_screen_mounts
                    ),
                );
            }
            passed.push("the Start Screen never mounted".to_string());
            match observation.workspaces.last() {
                Some(root) if same_dir(root, &expectation.workspace) => {}
                other => {
                    return fail(
                        "the recent workspace was opened",
                        format!(
                            "open workspace is {:?}, expected {:?}",
                            other, expectation.workspace
                        ),
                    );
                }
            }
            if dom.tree_rows < 2 {
                return fail(
                    "the workspace tree is shown",
                    format!(
                        "{} tree row(s), start screen in DOM: {}",
                        dom.tree_rows, dom.start_screen
                    ),
                );
            }
            passed.push(format!(
                "the workspace tree is shown ({} rows)",
                dom.tree_rows
            ));
            if !toast_texts.is_empty() {
                return fail("no reopen toast", format!("saw {toast_texts:?}"));
            }
            passed.push("no toast appeared".to_string());
        }
        ReleaseScenario::ReopenMissing | ReleaseScenario::ReopenDisabled => {
            if !observation.workspaces.is_empty() || observation.session_seen {
                let older = expectation
                    .older_workspace
                    .as_ref()
                    .filter(|older| observation.workspaces.iter().any(|w| same_dir(w, older)))
                    .map_or("", |_| " (the older recent entry)");
                return fail(
                    "no workspace opened",
                    format!(
                        "a workspace opened: {:?}{older}, document open: {}",
                        observation
                            .workspaces
                            .iter()
                            .map(opened)
                            .collect::<Vec<_>>(),
                        observation.session_seen
                    ),
                );
            }
            passed.push("no workspace opened".to_string());
            if !dom.start_screen || observation.start_screen_mounts == 0 {
                return fail(
                    "the Start Screen is shown",
                    format!(
                        "start screen in DOM: {}, mounts: {}, {} tree row(s){}",
                        dom.start_screen,
                        observation.start_screen_mounts,
                        dom.tree_rows,
                        observation
                            .dom_error
                            .as_ref()
                            .map_or(String::new(), |error| format!(", last DOM error: {error}"))
                    ),
                );
            }
            passed.push("the Start Screen is shown".to_string());
            if scenario == ReleaseScenario::ReopenDisabled {
                if !toast_texts.is_empty() {
                    return fail("no reopen toast", format!("saw {toast_texts:?}"));
                }
                passed.push("no toast appeared".to_string());
            } else {
                let expected = format!(
                    "{}: {}",
                    tr(lang, "workspace.reopen_failed"),
                    expectation.display_name
                );
                match toast_texts.as_slice() {
                    [only] if *only == format!("{:?}: {expected}", ToastKind::Warning) => {}
                    other => {
                        return fail(
                            "exactly one Warning toast naming the workspace",
                            format!(
                                "expected {:?}: {expected}, saw {other:?}",
                                ToastKind::Warning
                            ),
                        );
                    }
                }
                passed.push(format!("exactly one Warning toast: {expected}"));
            }
        }
        ReleaseScenario::SavePreservesBytes
        | ReleaseScenario::SavePreservesCrlfBytes
        | ReleaseScenario::ModeSwitchPreservesBytes
        | ReleaseScenario::LinkClicksReachOnlyTheBrowser
        | ReleaseScenario::PasteProbe => {
            unreachable!(
                "save.rs, mode_switch.rs, link_clicks.rs and paste_probe.rs run these scenarios"
            )
        }
    }
    Ok(passed)
}

fn sample(observation: &mut Observation, state: Signal<AppState>, toasts: Signal<Vec<Toast>>) {
    if let Ok(app) = state.try_peek() {
        if let Some(workspace) = &app.workspace
            && !observation.workspaces.contains(&workspace.root_path)
        {
            observation.workspaces.push(workspace.root_path.clone());
        }
        observation.session_seen |= app.session.is_some();
    }
    if let Ok(list) = toasts.try_peek() {
        for toast in list.iter() {
            observation
                .toasts
                .entry(toast.id)
                .or_insert_with(|| (toast.kind.clone(), toast.message.clone()));
        }
    }
    observation.start_screen_mounts = observation
        .start_screen_mounts
        .max(crate::webview_smoke::start_screen_mounts());
}

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    state: Signal<AppState>,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
) -> Result<Vec<String>, String> {
    let started = tokio::time::Instant::now();
    let mut observation = Observation::default();
    let mut ready_at = None;
    loop {
        sample(&mut observation, state, toasts);
        match dom::snapshot().await {
            Ok(snapshot) => observation.dom = Some(snapshot),
            Err(error) => observation.dom_error = Some(error),
        }
        if ready_at.is_none() && looks_ready(terminal.scenario, &observation) {
            ready_at = Some(tokio::time::Instant::now());
        }
        let watched = ready_at.is_some_and(|at| at.elapsed() >= WATCH_AFTER_READY);
        if watched || started.elapsed() >= DEADLINE {
            break;
        }
        tokio::time::sleep(POLL).await;
    }
    sample(&mut observation, state, toasts);
    judge(terminal.scenario, &terminal.expectation, lang, &observation)
}
