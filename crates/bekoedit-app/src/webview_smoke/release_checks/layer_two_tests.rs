// Task 035: the judgement of stage 2 of `link_clicks_reach_only_the_browser`.
// Every rule has a test that fails when the rule is removed; the mutations are
// in the review request.

use super::link_judge::{ClickObservation, MAIL_URL, WEB_URL};
use super::link_layer_two::{CONTROL_TEXT, REL_TEXT, StageTwo, judge_layer_two};
use crate::components::toast::ToastKind;

fn good() -> StageTwo {
    StageTwo {
        guard_removed: true,
        gone: ClickObservation::default(),
        isolated: ClickObservation::default(),
        control: ClickObservation {
            opener_lines: vec![WEB_URL.to_string()],
            ..Default::default()
        },
        final_log: vec![WEB_URL.into(), MAIL_URL.into(), WEB_URL.into()],
    }
}

fn notice() -> (ToastKind, String) {
    (ToastKind::Info, "Link not opened: something".to_string())
}

fn error_of(stage: &StageTwo) -> String {
    judge_layer_two(stage).expect_err("this observation must fail")
}

#[test]
fn a_correct_stage_passes_and_says_what_each_step_showed() {
    let passed = judge_layer_two(&good()).unwrap();
    assert_eq!(passed.len(), 4, "{passed:?}");
    assert!(passed[0].contains("step 1"));
    assert!(passed[1].contains("step 2") && passed[1].contains("layer 2 alone"));
    assert!(passed[2].contains("step 3") && passed[2].contains("the control"));
    assert!(passed[3].contains("the stub file holds exactly"));
}

#[test]
fn a_guard_that_was_never_there_fails_the_stage() {
    let mut stage = good();
    stage.guard_removed = false;
    let error = error_of(&stage);
    assert!(
        error.contains("stage 2") && error.contains("no guard listener to remove"),
        "{error}"
    );
}

#[test]
fn a_notice_after_the_removal_means_the_guard_is_still_there() {
    let mut stage = good();
    stage.gone.toasts.push(notice());
    let error = error_of(&stage);
    assert!(
        error.contains("step 1") && error.contains(REL_TEXT),
        "{error}"
    );
    assert!(error.contains("still active"), "{error}");
    assert!(error.contains("Link not opened: something"), "{error}");
}

#[test]
fn a_line_at_the_opener_in_step_one_fails_naming_it() {
    let mut stage = good();
    stage.gone.opener_lines.push("file:///cwd/other.md".into());
    let error = error_of(&stage);
    assert!(
        error.contains("step 1") && error.contains("file:///cwd/other.md"),
        "{error}"
    );
}

#[test]
fn a_leaked_line_in_step_two_fails_naming_the_stage_the_link_and_the_line() {
    let mut stage = good();
    stage
        .isolated
        .opener_lines
        .push("file:///cwd/other.md".into());
    let error = error_of(&stage);
    assert!(
        error.contains("stage 2") && error.contains("step 2"),
        "{error}"
    );
    assert!(error.contains(REL_TEXT), "{error}");
    assert!(error.contains("file:///cwd/other.md"), "{error}");
    assert!(error.contains("the toast layer showed"), "{error}");
    assert!(error.contains("interpreter's own link route"), "{error}");
}

#[test]
fn a_notice_in_step_two_fails() {
    let mut stage = good();
    stage.isolated.toasts.push(notice());
    let error = error_of(&stage);
    assert!(
        error.contains("step 2") && error.contains("raised a notice"),
        "{error}"
    );
}

#[test]
fn a_missing_control_line_says_stop_and_report_and_that_step_two_proves_nothing() {
    let mut stage = good();
    stage.control.opener_lines.clear();
    let error = error_of(&stage);
    assert!(
        error.contains("step 3") && error.contains(CONTROL_TEXT),
        "{error}"
    );
    assert!(error.contains("STOP AND REPORT"), "{error}");
    assert!(error.contains("step 2 above proves nothing"), "{error}");
}

#[test]
fn the_control_must_be_exactly_the_one_url_once() {
    let mut twice = good();
    twice.control.opener_lines = vec![WEB_URL.into(), WEB_URL.into()];
    assert!(error_of(&twice).contains("STOP AND REPORT"));

    let mut other = good();
    other.control.opener_lines = vec!["https://example.com/other".into()];
    assert!(error_of(&other).contains("STOP AND REPORT"));
}

#[test]
fn a_notice_on_the_control_fails() {
    let mut stage = good();
    stage.control.toasts.push(notice());
    let error = error_of(&stage);
    assert!(
        error.contains("step 3") && error.contains("raised a notice"),
        "{error}"
    );
}

#[test]
fn the_whole_file_must_be_stage_ones_two_lines_then_the_control() {
    let mut extra = good();
    extra.final_log.push("file:///cwd/other.md".into());
    let error = error_of(&extra);
    assert!(
        error.contains("nothing else") && error.contains("file:///cwd/other.md"),
        "{error}"
    );

    let mut missing = good();
    missing.final_log.pop();
    assert!(error_of(&missing).contains("the whole file was"));

    let mut swapped = good();
    swapped.final_log = vec![WEB_URL.into(), WEB_URL.into(), MAIL_URL.into()];
    assert!(error_of(&swapped).contains("the whole file was"));

    let mut fallback = good();
    fallback
        .final_log
        .insert(1, "UNEXPECTED-OPENER xdg-open x".into());
    assert!(error_of(&fallback).contains("UNEXPECTED-OPENER"));
}

#[test]
fn the_steps_are_judged_in_order() {
    // A failure in step 2 is reported as step 2 even if step 3 also failed:
    // the first thing that went wrong is the thing to look at.
    let mut stage = good();
    stage.isolated.opener_lines.push("file:///x".into());
    stage.control.opener_lines.clear();
    assert!(error_of(&stage).contains("step 2"));
}
