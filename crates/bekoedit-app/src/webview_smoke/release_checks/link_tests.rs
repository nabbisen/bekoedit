// Task 033: the judgement of `link_clicks_reach_only_the_browser`, and the
// fixture it clicks. Every rule of `judge` has a test that fails when the rule
// is removed; the mutations are in the review request.

use super::link_judge::{
    ClickObservation, DROPPED_TEXT, Expect, MAIL_URL, NAME, NOTE, Observation, RenderedLink, STEPS,
    UNEXPECTED_OPENER, WEB_URL, judge,
};
use crate::components::toast::ToastKind;
use crate::i18n::{Lang, tr};

const LANG: Lang = Lang::En;

fn rendered() -> Vec<RenderedLink> {
    STEPS
        .iter()
        .filter(|step| !step.injected)
        .map(|step| RenderedLink {
            text: step.text.to_string(),
            href: step.href.to_string(),
        })
        .collect()
}

/// What a correct run observes.
fn good() -> Observation {
    let clicks = STEPS
        .iter()
        .map(|step| match step.expect {
            Expect::Opens(url) => ClickObservation {
                opener_lines: vec![url.to_string()],
                ..Default::default()
            },
            Expect::Notice(key) => ClickObservation {
                toasts: vec![(ToastKind::Info, tr(LANG, key).to_string())],
                ..Default::default()
            },
            Expect::Nothing => ClickObservation::default(),
        })
        .collect();
    Observation {
        rendered: rendered(),
        clicks,
        final_log: vec![WEB_URL.to_string(), MAIL_URL.to_string()],
    }
}

fn index(text: &str) -> usize {
    STEPS.iter().position(|step| step.text == text).unwrap()
}

fn error_of(observation: &Observation) -> String {
    judge(LANG, observation).expect_err("this observation must fail")
}

#[test]
fn a_correct_run_passes_and_says_what_it_saw() {
    let passed = judge(LANG, &good()).unwrap();
    assert!(passed.iter().any(|line| line.contains("plain text")));
    for step in STEPS {
        assert!(
            passed.iter().any(|line| line.contains(step.text)),
            "{} missing from {passed:?}",
            step.text
        );
    }
    assert!(
        passed
            .last()
            .unwrap()
            .contains("the stub file holds exactly")
    );
}

#[test]
fn a_relative_link_that_reaches_the_opener_fails_naming_the_link_and_the_file_url() {
    let mut observation = good();
    observation.clicks[index("rel-file")].opener_lines = vec!["file:///cwd/other.md".into()];
    let error = error_of(&observation);
    assert!(
        error.contains(NAME) && error.contains("\"rel-file\""),
        "{error}"
    );
    assert!(error.contains("file:///cwd/other.md"), "{error}");
    assert!(error.contains("something reached an opener"), "{error}");
}

#[test]
fn a_refused_link_with_no_notice_fails_naming_the_link() {
    for text in ["rel-file", "injected-netpath"] {
        let mut observation = good();
        observation.clicks[index(text)].toasts.clear();
        let error = error_of(&observation);
        assert!(error.contains(&format!("{text:?}")), "{error}");
        assert!(
            error.contains("the notice was not raised exactly once"),
            "{error}"
        );
        assert!(error.contains("the toast layer showed []"), "{error}");
    }
}

#[test]
fn the_notice_must_be_the_right_one_once_and_an_info() {
    let mut wrong_text = good();
    wrong_text.clicks[index("rel-file")].toasts[0].1 = tr(LANG, "link.no_target").to_string();
    assert!(error_of(&wrong_text).contains("\"rel-file\""));

    let mut wrong_kind = good();
    wrong_kind.clicks[index("injected-netpath")].toasts[0].0 = ToastKind::Warning;
    assert!(error_of(&wrong_kind).contains("\"injected-netpath\""));

    let mut twice = good();
    let toast = twice.clicks[index("rel-file")].toasts[0].clone();
    twice.clicks[index("rel-file")].toasts.push(toast);
    assert!(error_of(&twice).contains("exactly once"));
}

#[test]
fn a_network_path_that_reaches_the_opener_fails() {
    let mut observation = good();
    observation.clicks[index("injected-netpath")].opener_lines = vec!["file:////host/x".into()];
    let error = error_of(&observation);
    assert!(error.contains("\"injected-netpath\""), "{error}");
    assert!(error.contains("file:////host/x"), "{error}");
}

#[test]
fn a_fragment_must_do_nothing_at_all() {
    let mut notice = good();
    notice.clicks[index("frag")]
        .toasts
        .push((ToastKind::Info, "x".into()));
    let error = error_of(&notice);
    assert!(
        error.contains("\"frag\"") && error.contains("a notice appeared"),
        "{error}"
    );

    let mut opener = good();
    opener.clicks[index("frag")].opener_lines = vec!["file:///cwd/#top".into()];
    assert!(error_of(&opener).contains("something reached an opener"));
}

#[test]
fn an_external_link_must_reach_the_opener_exactly_once_with_no_notice() {
    for (text, url) in [("web", WEB_URL), ("mail", MAIL_URL)] {
        let mut missing = good();
        missing.clicks[index(text)].opener_lines.clear();
        let error = error_of(&missing);
        assert!(error.contains(&format!("{text:?}")), "{error}");
        assert!(
            error.contains("did not receive exactly that URL"),
            "{error}"
        );

        let mut twice = good();
        twice.clicks[index(text)].opener_lines = vec![url.into(), url.into()];
        assert!(error_of(&twice).contains(&format!("{text:?}")));

        let mut other_url = good();
        other_url.clicks[index(text)].opener_lines = vec!["https://example.com/other".into()];
        assert!(error_of(&other_url).contains(&format!("{text:?}")));

        let mut noticed = good();
        noticed.clicks[index(text)]
            .toasts
            .push((ToastKind::Warning, "Could not open".into()));
        let error = error_of(&noticed);
        assert!(error.contains("a notice appeared"), "{error}");
        assert!(error.contains("Warning: Could not open"), "{error}");
    }
}

#[test]
fn a_third_line_in_the_stub_file_fails_naming_that_line() {
    let mut observation = good();
    observation.final_log.push("file:///cwd/other.md".into());
    let error = error_of(&observation);
    assert!(
        error.contains("line 3 is \"file:///cwd/other.md\""),
        "{error}"
    );
    assert!(error.contains("the whole file was"), "{error}");
    assert!(error.contains("the toast layer showed"), "{error}");
}

#[test]
fn a_fallback_opener_line_fails_and_says_which() {
    let mut observation = good();
    observation.final_log.insert(
        0,
        format!("{UNEXPECTED_OPENER} xdg-open file:///cwd/other.md"),
    );
    let error = error_of(&observation);
    assert!(error.contains("a fallback opener ran"), "{error}");
    assert!(error.contains("xdg-open"), "{error}");
}

#[test]
fn the_stub_file_must_hold_both_urls_in_click_order() {
    let mut short = good();
    short.final_log.pop();
    assert!(error_of(&short).contains("line 2 is missing"));

    let mut empty = good();
    empty.final_log.clear();
    assert!(error_of(&empty).contains("line 1 is missing"));

    let mut swapped = good();
    swapped.final_log.reverse();
    let error = error_of(&swapped);
    assert!(
        error.contains("line 1 is") && error.contains("expected"),
        "{error}"
    );
}

#[test]
fn the_network_path_link_must_render_as_plain_text() {
    let mut observation = good();
    observation.rendered.push(RenderedLink {
        text: DROPPED_TEXT.into(),
        href: "//host/x".into(),
    });
    let error = error_of(&observation);
    assert!(error.contains("unexpected link"), "{error}");
    assert!(error.contains(DROPPED_TEXT), "{error}");
}

#[test]
fn every_real_link_must_render_with_its_href() {
    for step in STEPS.iter().filter(|step| !step.injected) {
        let mut missing = good();
        missing.rendered.retain(|link| link.text != step.text);
        let error = error_of(&missing);
        assert!(error.contains(&format!("{:?}", step.text)), "{error}");
        assert!(error.contains("did not render"), "{error}");

        let mut wrong_href = good();
        for link in &mut wrong_href.rendered {
            if link.text == step.text {
                link.href = "elsewhere.md".into();
            }
        }
        assert!(error_of(&wrong_href).contains("did not render"));
    }
}

#[test]
fn one_observation_per_click_is_required() {
    let mut observation = good();
    observation.clicks.pop();
    assert!(error_of(&observation).contains("click(s) were observed"));
}

#[test]
fn the_plan_only_expects_an_open_for_web_and_mail_urls() {
    for step in STEPS {
        if let Expect::Opens(url) = step.expect {
            assert!(
                url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("mailto:"),
                "{step:?}"
            );
            assert_eq!(url, step.href, "an open is of the link's own href");
        }
    }
    assert_eq!(
        STEPS.iter().filter(|step| step.injected).count(),
        1,
        "only the network-path link is injected"
    );
}

/// The fixture is what the scenario assumes, according to the real renderer:
/// the four links are anchors with their `href`s, and the network-path link is
/// plain text (task 030).
#[test]
fn the_fixture_renders_four_anchors_and_drops_the_network_path_link() {
    let html = bekoedit_markdown::render_preview_html(NOTE);
    for step in STEPS.iter().filter(|step| !step.injected) {
        let anchor = format!("<a href=\"{}\">{}</a>", step.href, step.text);
        assert!(html.contains(&anchor), "{anchor} missing from {html}");
    }
    assert_eq!(html.matches("<a ").count(), 4, "{html}");
    assert!(!html.contains("//host/x"), "{html}");
    assert!(html.contains(DROPPED_TEXT), "{html}");
}

#[test]
fn the_new_scenario_needs_the_opener_log_from_the_environment() {
    use super::ReleaseScenario;
    use super::seed::{OPENER_LOG_ENV, prepare};
    if std::env::var_os(OPENER_LOG_ENV).is_some() {
        return; // A developer's own shell; the assertion below is for the unset case.
    }
    let scenario = ReleaseScenario::parse(NAME).unwrap();
    let root = std::env::temp_dir().join(format!(
        "bekoedit-webview-smoke-test-link-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let error = prepare(&root, scenario)
        .err()
        .expect("must not seed without a log");
    assert!(
        error.contains(OPENER_LOG_ENV) && error.contains(NAME),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
