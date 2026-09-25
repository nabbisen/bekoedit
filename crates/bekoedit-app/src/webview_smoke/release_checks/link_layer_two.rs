//! Task 035: the final stage of `link_clicks_reach_only_the_browser`, which
//! proves task 032's **second layer** on its own.
//!
//! In the first five clicks the capture-phase guard (layer 1) catches every
//! click, so Dioxus's own link route being switched off (layer 2) is never
//! exercised: layer 2 could regress and the scenario would still pass. This
//! stage takes layer 1 away, in the page, and clicks again:
//!
//! 1. **The guard is removed** (its listener, in the page). A click on
//!    `rel-file` must then raise **no notice**, because nothing sends it to
//!    Rust any more. That confirms the removal by what it changes.
//! 2. **Layer 2 alone.** A second click on `rel-file`: **nothing** may reach the
//!    opener. With the interpreter's route off, it is an ordinary navigation to
//!    a `dioxus://` URL, which wry refuses. With the flag regressed, the
//!    interpreter would send the raw `href` to `webbrowser::open`, and the stub
//!    would log a `file://` line.
//! 3. **The control.** A click on `web`: with the guard gone it must still
//!    reach the stub, **exactly one new line, `https://example.com/x`**, through
//!    wry's own navigation handler. Without this, step 2 proves nothing: it
//!    would also pass if every route were dead. If this fails on the first real
//!    run, the design's reading of wry is wrong, and the run must be reported,
//!    not made to pass by weakening step 2.
//!
//! Pure judgement only. The driver is `link_clicks.rs`.

use super::link_judge::{ClickObservation, MAIL_URL, NAME, WEB_URL};

pub(super) const REL_TEXT: &str = "rel-file";
pub(super) const CONTROL_TEXT: &str = "web";

#[derive(Debug, Default)]
pub(super) struct StageTwo {
    /// Whether the page had a guard listener to remove.
    pub guard_removed: bool,
    /// Step 1: the click on `rel-file` right after the removal.
    pub gone: ClickObservation,
    /// Step 2: the second click on `rel-file`.
    pub isolated: ClickObservation,
    /// Step 3: the click on `web`.
    pub control: ClickObservation,
    /// The stub file, line by line, after all three and a settle window.
    pub final_log: Vec<String>,
}

fn shown(click: &ClickObservation) -> String {
    let toasts: Vec<String> = click
        .toasts
        .iter()
        .map(|(kind, message)| format!("{kind:?}: {message}"))
        .collect();
    format!(
        "the toast layer showed {toasts:?}; the stub file gained {:?}",
        click.opener_lines
    )
}

/// Judges stage 2. Every failure names the stage, the step, the link, and what
/// the stub file and the toast layer showed. Pure, so each rule is unit tested
/// and mutation-proven.
pub(super) fn judge_layer_two(observation: &StageTwo) -> Result<Vec<String>, String> {
    let stage = format!("{NAME}: stage 2 (layer two in isolation)");
    let mut passed = Vec::new();

    if !observation.guard_removed {
        return Err(format!(
            "{stage}, step 1: there was no guard listener to remove (`window.__bk_link_guard` \
             is not a function), so the guard never installed and layer 2 cannot be tested alone"
        ));
    }

    // Step 1: the removal, confirmed by what it changes.
    if !observation.gone.toasts.is_empty() {
        return Err(format!(
            "{stage}, step 1 (guard removed): a click on {REL_TEXT:?} still raised a notice, so \
             the guard's listener is still active or Rust was reached another way; {}",
            shown(&observation.gone)
        ));
    }
    if !observation.gone.opener_lines.is_empty() {
        return Err(format!(
            "{stage}, step 1 (guard removed): a click on {REL_TEXT:?} reached the opener; {}",
            shown(&observation.gone)
        ));
    }
    passed.push(format!(
        "stage 2 step 1: the guard's listener was removed, and a click on {REL_TEXT:?} raised no \
         notice"
    ));

    // Step 2: layer 2 alone.
    if !observation.isolated.opener_lines.is_empty() {
        return Err(format!(
            "{stage}, step 2 (layer 2 alone): a click on {REL_TEXT:?} reached the opener with \
             the guard gone. The interpreter's own link route is still on, or something else \
             opens; {}",
            shown(&observation.isolated)
        ));
    }
    if !observation.isolated.toasts.is_empty() {
        return Err(format!(
            "{stage}, step 2 (layer 2 alone): a click on {REL_TEXT:?} raised a notice with the \
             guard gone; {}",
            shown(&observation.isolated)
        ));
    }
    passed.push(format!(
        "stage 2 step 2: with the guard gone, a click on {REL_TEXT:?} reached no opener and \
         raised no notice (layer 2 alone)"
    ));

    // Step 3: the control, without which step 2 proves nothing.
    if observation.control.opener_lines != [WEB_URL] {
        return Err(format!(
            "{stage}, step 3 (control): STOP AND REPORT (task 035 §2). A click on \
             {CONTROL_TEXT:?} with the guard gone did not reach the stub as exactly {WEB_URL:?}, \
             so wry's navigation handler is not opening `https:` there as expected, and step 2 \
             above proves nothing (it would also pass if every route were dead); {}",
            shown(&observation.control)
        ));
    }
    if !observation.control.toasts.is_empty() {
        return Err(format!(
            "{stage}, step 3 (control): a click on {CONTROL_TEXT:?} raised a notice with the \
             guard gone; {}",
            shown(&observation.control)
        ));
    }
    passed.push(format!(
        "stage 2 step 3: with the guard gone, a click on {CONTROL_TEXT:?} still reached the stub \
         as exactly {WEB_URL:?}, through wry's own handler (the control)"
    ));

    // The whole file: stage 1's two lines, then the control's.
    let wanted = [WEB_URL, MAIL_URL, WEB_URL];
    let log = &observation.final_log;
    if log != &wanted {
        return Err(format!(
            "{stage}: the stub file must hold exactly {wanted:?} and nothing else; the whole \
             file was {log:?}"
        ));
    }
    passed.push(format!("stage 2: the stub file holds exactly {wanted:?}"));
    Ok(passed)
}
