//! Task 033, `link_clicks_reach_only_the_browser`: the driver. What is clicked
//! and how the result is judged is in `link_judge.rs`.
//!
//! Opens the fixture note in Preview through the tree with a real click, then
//! sends a real XTEST click to each link in `STEPS`, watching the toast layer
//! and the opener stub's log for a settle window after each, and once more
//! after the last. The stub and its log are made by
//! `scripts/webview-link-opener-stubs.sh`, which CI runs before the launch and
//! whose log path arrives in `BEKOEDIT_LINK_OPENER_LOG`.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;

use crate::components::toast::Toast;
use crate::i18n::Lang;
use crate::webview_smoke::trusted_click::xtest::{activate_window, click_via_xtest};

use super::ReleaseChecksTerminal;
use super::dom;
use super::link_judge::{
    ClickObservation, NAME, NOTE_FILE, Observation, RenderedLink, STEPS, judge,
};
use super::save::wait_until;

/// After a click, how long the toast layer and the stub log are watched. The
/// same span task 026's launch scenarios watch after they look ready
/// (`WATCH_AFTER_READY`): far longer than a click, the guard's round trip to
/// Rust and a spawned stub take, so a leak that would come has come.
const SETTLE_WINDOW: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_millis(40);

fn read_log(log: &Path) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(log) {
        Ok(text) => Ok(text.lines().map(str::to_string).collect()),
        // The stubs create the file; one that never ran leaves none.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!(
            "{NAME}: cannot read the opener log {}: {error}",
            log.display()
        )),
    }
}

/// Watches for `SETTLE_WINDOW`, and returns the toasts that were not there
/// before and the log lines added since `lines_before`.
async fn watch(
    toasts: Signal<Vec<Toast>>,
    seen: &mut BTreeSet<u64>,
    log: &Path,
    lines_before: usize,
) -> Result<ClickObservation, String> {
    let mut observation = ClickObservation::default();
    let started = tokio::time::Instant::now();
    loop {
        if let Ok(list) = toasts.try_peek() {
            for toast in list.iter() {
                if seen.insert(toast.id) {
                    observation
                        .toasts
                        .push((toast.kind.clone(), toast.message.clone()));
                }
            }
        }
        if started.elapsed() >= SETTLE_WINDOW {
            break;
        }
        tokio::time::sleep(POLL).await;
    }
    let lines = read_log(log)?;
    observation.opener_lines = lines.get(lines_before..).unwrap_or_default().to_vec();
    Ok(observation)
}

pub(super) async fn run(
    terminal: &ReleaseChecksTerminal,
    desktop: &DesktopContext,
    toasts: Signal<Vec<Toast>>,
    lang: Lang,
) -> Result<Vec<String>, String> {
    let log = terminal
        .expectation
        .opener_log
        .as_ref()
        .ok_or_else(|| format!("{NAME}: no opener log was configured"))?;
    let stale = read_log(log)?;
    if !stale.is_empty() {
        return Err(format!(
            "{NAME}: the opener log already held {stale:?} before any click; it must start empty"
        ));
    }

    wait_until(
        NAME,
        "the workspace tree to show the seeded files",
        || async { Ok(dom::snapshot().await?.tree_rows >= 2) },
    )
    .await?;
    activate_window(desktop);
    click_via_xtest(desktop, ".tree-row.tree-file", Some(NOTE_FILE), 0).await?;
    wait_until(NAME, "the note to render its links in Preview", || async {
        Ok(dom::preview_links().await?.len() >= 4)
    })
    .await?;
    let rendered = dom::preview_links()
        .await?
        .into_iter()
        .map(|link| RenderedLink {
            text: link.text,
            href: link.href,
        })
        .collect();

    let mut observation = Observation {
        rendered,
        ..Default::default()
    };
    let mut seen: BTreeSet<u64> = toasts
        .try_peek()
        .map(|list| list.iter().map(|toast| toast.id).collect())
        .unwrap_or_default();
    let mut lines_before = read_log(log)?.len();
    for step in STEPS {
        if step.injected && !dom::inject_preview_anchor(step.text, step.href).await? {
            return Err(format!(
                "{NAME}: there is no Preview article to add {:?} to",
                step.text
            ));
        }
        click_via_xtest(desktop, "article.preview a", Some(step.text), 0).await?;
        let click = watch(toasts, &mut seen, log, lines_before).await?;
        lines_before += click.opener_lines.len();
        observation.clicks.push(click);
    }
    // After the last click, one more window: the assertion is on the whole file.
    tokio::time::sleep(SETTLE_WINDOW).await;
    observation.final_log = read_log(log)?;
    judge(lang, &observation)
}
