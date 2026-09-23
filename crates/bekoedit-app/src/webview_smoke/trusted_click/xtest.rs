//! The real XTEST click mechanics task 023 exists for: locating a click
//! target's on-page rect, converting it to a physical screen point, and
//! sending the click through `xdotool`. Split out of `trusted_click.rs`
//! to keep that file under the project's 300-ELOC split guideline
//! (`.github/CONTRIBUTING.md`) -- the same reason `shell_behaviour.rs`
//! keeps its phase table in its own `phase.rs`.

use dioxus::desktop::DesktopContext;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::phase::TrustedClickPhase;

const LOCATE_TIMEOUT_MS: u64 = 5000;

/// Centre of a CSS-pixel bounding rect, in the same units. A separate,
/// pure function so it can be unit tested without a real window or
/// element (task 023 §5.4).
pub(super) fn rect_center(x: f64, y: f64, width: f64, height: f64) -> (f64, f64) {
    (x + width / 2.0, y + height / 2.0)
}

/// Converts a CSS-pixel point inside the WebView content area to a
/// physical screen point `xdotool` can click: the window's own physical
/// top-left (`Window::inner_position`) plus the point scaled by
/// `Window::scale_factor`. A separate, pure function so the rounding and
/// scaling can be unit tested without a real window (task 023 §5.4).
pub(super) fn screen_point(
    inner_position: (i32, i32),
    scale_factor: f64,
    css_point: (f64, f64),
) -> (i32, i32) {
    let (origin_x, origin_y) = inner_position;
    let (css_x, css_y) = css_point;
    (
        origin_x + (css_x * scale_factor).round() as i32,
        origin_y + (css_y * scale_factor).round() as i32,
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocateRequest<'a> {
    selector: &'a str,
    text_includes: Option<&'a str>,
    index: usize,
    timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocateResponse {
    found: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// The assembled JS for `locate_click_target`, given its already-encoded
/// JSON `payload` -- split out from `locate_click_target` itself so the
/// assembly (an unbalanced paren or brace here throws a silent
/// `SyntaxError` in the WebView -- no Rust gate over the fragments it's
/// built from would catch that, per the RFC-042 slice 3 re-review C1/C2
/// finding `shell_focus/tests.rs` documents) can be unit tested without a
/// real WebView (task 023 §5.4).
///
/// Requires the same rect across two consecutive polls before accepting
/// it: CI's fifth real run showed the first nonzero rect a freshly
/// launched WebView reports for `#app-menu-trigger` can be a pre-layout
/// position (`(8, 32, 29, 24)`) that the page immediately abandons for
/// its true, stable one (`(1159, 6, 31, 23)`, seen once a slower run's
/// own repeated polling happened to land after layout settled) -- a
/// single nonzero reading is not evidence the layout has finished.
fn render_locate_script(payload: &str) -> String {
    format!(
        r#"
        return (async () => {{
            const request = {payload};
            const deadline = performance.now() + request.timeoutMs;
            const sameRect = (a, b) =>
                a !== null && b !== null &&
                a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
            let rect = null;
            let previous = null;
            while (performance.now() < deadline) {{
                const matches = [...document.querySelectorAll(request.selector)].filter(
                    (candidate) =>
                        request.textIncludes === null ||
                        candidate.textContent.includes(request.textIncludes),
                );
                const el = matches[request.index] ?? null;
                const current = el && el.getClientRects().length > 0
                    ? el.getBoundingClientRect()
                    : null;
                if (current && sameRect(current, previous)) {{
                    rect = current;
                    break;
                }}
                previous = current;
                await new Promise((resolve) => setTimeout(resolve, 50));
            }}
            if (!rect) {{
                dioxus.send({{ found: false, x: 0, y: 0, width: 0, height: 0 }});
                return;
            }}
            dioxus.send({{
                found: true,
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            }});
        }})();
        "#,
    )
}

/// Fetches one element's `getBoundingClientRect()` through a bespoke,
/// single-exchange `document::eval` -- not the shared phase transport,
/// per `trusted_click.rs`'s own doc comment. `selector` may match more
/// than one element (`index` picks among them); `text_includes`, when
/// given, keeps only elements whose `textContent` contains it, since a
/// workspace-tree row or menu item carries no selector more specific than
/// its rendered text (surveyed against `tree_row.rs` and
/// `editor_header.rs` while designing this run -- see the review
/// request).
async fn locate_click_target(
    selector: &str,
    text_includes: Option<&str>,
    index: usize,
) -> Result<LocateResponse, String> {
    let payload = serde_json::to_string(&LocateRequest {
        selector,
        text_includes,
        index,
        timeout_ms: LOCATE_TIMEOUT_MS,
    })
    .map_err(|error| format!("cannot encode locate request for {selector}: {error}"))?;
    let mut eval = document::eval(&render_locate_script(&payload));
    eval.recv::<LocateResponse>()
        .await
        .map_err(|error| format!("could not locate {selector}: {error}"))
}

/// Runs an `xdotool` subcommand, logging its exit status and both streams
/// unconditionally -- task 023's first real CI run timed out at the very
/// first click with no other signal, so this run's own log is the only
/// diagnostic available while the soak (§4) is still open. Kept until the
/// mechanism is trusted; the review request says which CI runs this
/// covered.
fn run_xdotool(args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new("xdotool")
        .args(args)
        .output()
        .map_err(|error| format!("cannot spawn xdotool {args:?}: {error}"))?;
    println!(
        "  xdotool {args:?} -> {} stdout={:?} stderr={:?}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    if !output.status.success() {
        return Err(format!("xdotool {args:?} exited with {}", output.status));
    }
    Ok(())
}

/// Gives the bekoedit window X input focus by asking `tao` to do it from
/// inside the process that owns the window (`Window::set_focus`), not
/// through `xdotool` as an outside client. CI's first two real runs tried
/// `xdotool windowactivate` (fails outright: Xvfb runs no window manager
/// to answer its `_NET_ACTIVE_WINDOW` message) and then `xdotool
/// windowfocus` (fails with `X_SetInputFocus BadMatch` on every attempt,
/// not only the first -- not a startup race, but this run's window never
/// becoming one an *outside* client may focus without a window manager to
/// broker it). A client requesting focus for its own window is a
/// different, permitted request. Logs `is_focused()` either way and never
/// fails the run over it -- a real XTEST click is still delivered to
/// whatever window is under the pointer regardless. Idempotent; called
/// before every phase's clicks.
pub(super) fn activate_window(desktop: &DesktopContext) {
    desktop.window.set_focus();
    println!(
        "  trusted click: window focus requested, is_focused={}",
        desktop.window.is_focused()
    );
}

/// Locates one element, then sends a real XTEST click at its centre
/// through `xdotool` -- the one new CI dependency task 023 §3.4 allows.
/// Never a synthetic `.click()`: that is the entire point of this run.
async fn click_via_xtest(
    desktop: &DesktopContext,
    selector: &str,
    text_includes: Option<&str>,
    index: usize,
) -> Result<(), String> {
    let target = locate_click_target(selector, text_includes, index).await?;
    if !target.found {
        return Err(format!("trusted-click target not found: {selector}"));
    }
    let inner_position = desktop
        .window
        .inner_position()
        .map_err(|error| format!("cannot read window position for a trusted click: {error}"))?;
    let scale_factor = desktop.window.scale_factor();
    let (screen_x, screen_y) = screen_point(
        (inner_position.x, inner_position.y),
        scale_factor,
        rect_center(target.x, target.y, target.width, target.height),
    );
    println!(
        "  trusted click at {selector} (text_includes={text_includes:?}, index={index}): \
         rect=({}, {}, {}, {}) inner_position=({}, {}) scale_factor={scale_factor} \
         -> screen=({screen_x}, {screen_y})",
        target.x, target.y, target.width, target.height, inner_position.x, inner_position.y,
    );
    run_xdotool(&[
        "mousemove",
        "--sync",
        &screen_x.to_string(),
        &screen_y.to_string(),
        "click",
        "--clearmodifiers",
        "1",
    ])
}

/// The real XTEST click(s) this run performs before requesting `phase`'s
/// exchange -- `shell_behaviour.rs`'s `writes_conflict_after` pattern,
/// applied to input instead of a file write. Every phase has exactly one
/// click under test; `BacklinkFocus` also needs one real click first, to
/// open the backlinks panel -- real, not synthetic, per
/// `trusted_click.rs`'s own doc comment.
pub(super) async fn perform_trusted_clicks(
    desktop: &DesktopContext,
    phase: TrustedClickPhase,
) -> Result<(), String> {
    // Idempotent, and cheap next to a real click -- simpler to call before
    // every phase than to track "only the first click needs this".
    activate_window(desktop);
    match phase {
        TrustedClickPhase::ProofOfTrust => {
            click_via_xtest(desktop, "#app-menu-trigger", None, 0).await
        }
        TrustedClickPhase::TreeRowFocus => {
            click_via_xtest(desktop, ".tree-row.tree-file", Some("child.md"), 0).await
        }
        TrustedClickPhase::BacklinkFocus => {
            click_via_xtest(desktop, "#editor-tools-trigger", None, 0).await?;
            // Outline, Backlinks (this fixture always has one), History, in
            // that order -- the only three `.dropdown-item`s in this menu
            // without a `data-source-focus-launch` id (`editor_header.rs`).
            click_via_xtest(
                desktop,
                r#"#editor-tools-menu .dropdown-item[role="menuitem"]:not([data-source-focus-launch])"#,
                None,
                1,
            )
            .await?;
            click_via_xtest(
                desktop,
                r#"[data-source-focus-launch^="backlink:"]"#,
                None,
                0,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same check as `shell_focus/tests.rs`'s own `is_balanced`, against
    /// this module's own assembled template -- that file's coverage is
    /// scoped to `shell_focus`'s eval scripts, not this one.
    fn is_balanced(script: &str) -> bool {
        let mut parens = 0i32;
        let mut braces = 0i32;
        for c in script.chars() {
            match c {
                '(' => parens += 1,
                ')' => parens -= 1,
                '{' => braces += 1,
                '}' => braces -= 1,
                _ => {}
            }
            if parens < 0 || braces < 0 {
                return false;
            }
        }
        parens == 0 && braces == 0
    }

    #[test]
    fn render_locate_script_stays_balanced_with_a_real_payload() {
        let payload = serde_json::to_string(&LocateRequest {
            selector: ".tree-row.tree-file",
            text_includes: Some("child.md"),
            index: 0,
            timeout_ms: LOCATE_TIMEOUT_MS,
        })
        .unwrap();
        assert!(is_balanced(&render_locate_script(&payload)));
    }

    #[test]
    fn rect_center_is_the_midpoint() {
        assert_eq!(rect_center(10.0, 20.0, 30.0, 40.0), (25.0, 40.0));
        assert_eq!(rect_center(0.0, 0.0, 0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn screen_point_adds_the_scaled_css_point_to_the_window_origin() {
        // scale_factor 1: no scaling, just the window's own physical offset.
        assert_eq!(screen_point((100, 50), 1.0, (10.0, 20.0)), (110, 70));
        // scale_factor 2 (a HiDPI display): the CSS point is doubled before
        // being added, since getBoundingClientRect() reports CSS pixels but
        // xdotool clicks physical screen pixels.
        assert_eq!(screen_point((100, 50), 2.0, (10.0, 20.5)), (120, 91));
        // A fractional physical pixel rounds rather than truncates.
        assert_eq!(screen_point((0, 0), 1.5, (1.0, 1.0)), (2, 2));
    }
}
