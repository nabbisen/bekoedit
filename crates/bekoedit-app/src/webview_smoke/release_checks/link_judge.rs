//! Task 033, `link_clicks_reach_only_the_browser`: what is clicked, what each
//! click must do, and the pure judgement of what was observed.
//!
//! The scenario opens a fixture note in Preview and sends a real XTEST click to
//! each link. **Only `http(s):` and `mailto:` may reach an OS opener.** The
//! opener is a stub (`scripts/webview-link-opener-stubs.sh`): `$BROWSER` points
//! at one that appends its argument to a log, and every other opener
//! `webbrowser` could fall back to on Linux is shimmed on `PATH` to log
//! `UNEXPECTED-OPENER <name> <args>`. So the log is the whole truth about what
//! reached an opener, and the toast layer is the whole truth about which clicks
//! were refused with a notice.
//!
//! Nothing here touches the WebView. `judge` takes what was observed and
//! names the link, the stub log and the toast layer in every failure.

use crate::components::toast::ToastKind;
use crate::i18n::{Lang, tr};

pub(super) const NAME: &str = "link_clicks_reach_only_the_browser";
pub(super) const WEB_URL: &str = "https://example.com/x";
pub(super) const MAIL_URL: &str = "mailto:someone@example.com";
/// What each fallback shim writes before its own name.
pub(super) const UNEXPECTED_OPENER: &str = "UNEXPECTED-OPENER";

/// The fixture note. `[netpath](//host/x)` is written as Markdown, and task
/// 030's render policy drops it to plain text: it is not a link, so it cannot
/// be clicked. The click policy's answer to a network-path `href` is exercised
/// on an anchor the driver injects (`STEPS`, the `injected` one).
pub(super) const NOTE: &str = "# Links\n\n\
[rel-file](other.md)\n\n\
[frag](#top)\n\n\
[web](https://example.com/x)\n\n\
[netpath](//host/x)\n\n\
[mail](mailto:someone@example.com)\n";
pub(super) const NOTE_FILE: &str = "note.md";
pub(super) const OTHER_FILE: &str = "other.md";
/// The one Markdown link that must render as plain text (task 030's layer).
pub(super) const DROPPED_TEXT: &str = "netpath";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Expect {
    /// Exactly this URL reaches the opener, and no notice appears.
    Opens(&'static str),
    /// One Info notice with this i18n key appears, and nothing reaches an opener.
    Notice(&'static str),
    /// Nothing at all: no notice, no opener.
    Nothing,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Step {
    /// Also what the locator matches in the anchor's text.
    pub text: &'static str,
    pub href: &'static str,
    /// Not in the rendered note: the driver adds it to the Preview DOM.
    pub injected: bool,
    pub expect: Expect,
}

pub(super) const STEPS: [Step; 5] = [
    Step {
        text: "rel-file",
        href: "other.md",
        injected: false,
        expect: Expect::Notice("link.not_followed"),
    },
    Step {
        text: "frag",
        href: "#top",
        injected: false,
        expect: Expect::Nothing,
    },
    Step {
        text: "web",
        href: WEB_URL,
        injected: false,
        expect: Expect::Opens(WEB_URL),
    },
    Step {
        text: "injected-netpath",
        href: "//host/x",
        injected: true,
        expect: Expect::Notice("link.network_path"),
    },
    Step {
        text: "mail",
        href: MAIL_URL,
        injected: false,
        expect: Expect::Opens(MAIL_URL),
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RenderedLink {
    pub text: String,
    pub href: String,
}

/// What one click was followed by, over the settle window after it.
#[derive(Debug, Clone, Default)]
pub(super) struct ClickObservation {
    pub toasts: Vec<(ToastKind, String)>,
    pub opener_lines: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct Observation {
    /// The anchors in the rendered note, before anything was injected.
    pub rendered: Vec<RenderedLink>,
    /// One per `STEPS` entry, in order.
    pub clicks: Vec<ClickObservation>,
    /// The stub log, line by line, after every click and a final settle.
    pub final_log: Vec<String>,
}

fn describe(expect: Expect, lang: Lang) -> String {
    match expect {
        Expect::Opens(url) => format!("the opener to get exactly {url:?} and no notice"),
        Expect::Notice(key) => format!(
            "one Info notice {:?} and nothing at the opener",
            tr(lang, key)
        ),
        Expect::Nothing => "no notice and nothing at the opener".to_string(),
    }
}

fn shown_toasts(toasts: &[(ToastKind, String)]) -> Vec<String> {
    toasts
        .iter()
        .map(|(kind, message)| format!("{kind:?}: {message}"))
        .collect()
}

/// Judges an observation. Every failure names the link and what the stub log
/// and the toast layer showed. Pure, so each rule is unit tested and
/// mutation-proven.
pub(super) fn judge(lang: Lang, observation: &Observation) -> Result<Vec<String>, String> {
    let mut passed = Vec::new();

    // Task 030's layer: the four real links render, and `netpath` does not.
    let real: Vec<&Step> = STEPS.iter().filter(|step| !step.injected).collect();
    for step in &real {
        let want = RenderedLink {
            text: step.text.to_string(),
            href: step.href.to_string(),
        };
        if !observation.rendered.contains(&want) {
            return Err(format!(
                "{NAME}: the note did not render {:?} as a link to {:?}; rendered links: {:?}",
                step.text, step.href, observation.rendered
            ));
        }
    }
    if let Some(extra) = observation.rendered.iter().find(|link| {
        !real
            .iter()
            .any(|step| step.text == link.text && step.href == link.href)
    }) {
        return Err(format!(
            "{NAME}: the note rendered an unexpected link {extra:?}; {DROPPED_TEXT:?} \
             (`//host/x`) must be plain text, and only the {} real links may be anchors",
            real.len()
        ));
    }
    passed.push(format!(
        "{} links rendered as anchors; the network-path link {DROPPED_TEXT:?} is plain text (task 030)",
        real.len()
    ));

    if observation.clicks.len() != STEPS.len() {
        return Err(format!(
            "{NAME}: {} click(s) were observed, expected {}",
            observation.clicks.len(),
            STEPS.len()
        ));
    }
    for (step, click) in STEPS.iter().zip(&observation.clicks) {
        let fail = |what: &str| {
            Err(format!(
                "{NAME}: click on {:?} (href {:?}): expected {}; {what}; the toast layer showed \
                 {:?}; the stub file gained {:?}",
                step.text,
                step.href,
                describe(step.expect, lang),
                shown_toasts(&click.toasts),
                click.opener_lines,
            ))
        };
        match step.expect {
            Expect::Opens(url) => {
                if click.opener_lines != [url] {
                    return fail("the opener did not receive exactly that URL");
                }
                if !click.toasts.is_empty() {
                    return fail("a notice appeared");
                }
            }
            Expect::Notice(key) => {
                if !click.opener_lines.is_empty() {
                    return fail("something reached an opener");
                }
                let want = (ToastKind::Info, tr(lang, key).to_string());
                if click.toasts != [want] {
                    return fail("the notice was not raised exactly once");
                }
            }
            Expect::Nothing => {
                if !click.opener_lines.is_empty() {
                    return fail("something reached an opener");
                }
                if !click.toasts.is_empty() {
                    return fail("a notice appeared");
                }
            }
        }
        passed.push(format!(
            "{:?} ({}): {}",
            step.text,
            step.href,
            describe(step.expect, lang)
        ));
    }

    // The whole log, after every click and a settle window.
    let wanted = [WEB_URL, MAIL_URL];
    let log = &observation.final_log;
    let all_toasts: Vec<String> = observation
        .clicks
        .iter()
        .flat_map(|click| shown_toasts(&click.toasts))
        .collect();
    let log_fail = |what: String| {
        Err(format!(
            "{NAME}: the stub file must hold exactly {wanted:?} and nothing else: {what}; \
             the whole file was {log:?}; the toast layer showed {all_toasts:?}"
        ))
    };
    if let Some(line) = log.iter().find(|line| line.starts_with(UNEXPECTED_OPENER)) {
        return log_fail(format!("a fallback opener ran: {line:?}"));
    }
    if log.len() > wanted.len() {
        return log_fail(format!(
            "line {} is {:?}",
            wanted.len() + 1,
            log[wanted.len()]
        ));
    }
    for (index, want) in wanted.iter().enumerate() {
        match log.get(index) {
            Some(line) if line == want => {}
            Some(line) => {
                return log_fail(format!("line {} is {line:?}, expected {want:?}", index + 1));
            }
            None => return log_fail(format!("line {} is missing, expected {want:?}", index + 1)),
        }
    }
    passed.push(format!("the stub file holds exactly {wanted:?}"));
    Ok(passed)
}
