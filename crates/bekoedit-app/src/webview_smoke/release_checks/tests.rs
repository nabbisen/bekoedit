use std::path::PathBuf;

use super::bytes::{check_bytes_unchanged, check_saved_bytes};
use super::launch::{Observation, judge};
use super::seed::{EDIT_MARKER, original_crlf_note, original_note, prepare};
use super::*;
use crate::components::toast::ToastKind;
use crate::i18n::tr;
use crate::webview_smoke::release_checks::dom::DomSnapshot;

fn scratch(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bekoedit-webview-smoke-test-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn expectation() -> Expectation {
    Expectation {
        workspace: PathBuf::from("/ws/gone"),
        display_name: "Gone Project".into(),
        older_workspace: Some(PathBuf::from("/ws/older")),
        file: None,
        original: Vec::new(),
        opener_log: None,
    }
}

fn observed(start_screen: bool, mounts: usize, rows: usize) -> Observation {
    Observation {
        start_screen_mounts: mounts,
        dom: Some(DomSnapshot {
            start_screen,
            tree_rows: rows,
        }),
        ..Default::default()
    }
}

fn with_toast(mut observation: Observation, message: String, kind: ToastKind) -> Observation {
    let id = observation.toasts.len() as u64 + 1;
    observation.toasts.insert(id, (kind, message));
    observation
}

fn saved_with_marker(at: usize) -> Vec<u8> {
    let original = original_note();
    let mut saved = original[..at].to_vec();
    saved.extend_from_slice(EDIT_MARKER.as_bytes());
    saved.extend_from_slice(&original[at..]);
    saved
}

#[test]
fn scenarios_parse_by_name_and_reject_anything_else() {
    for name in [
        "reopen_usable",
        "reopen_missing",
        "reopen_disabled",
        "save_preserves_bytes",
        "save_preserves_crlf_bytes",
        "mode_switch_preserves_bytes",
        "link_clicks_reach_only_the_browser",
    ] {
        assert_eq!(ReleaseScenario::parse(name).unwrap().name(), name);
    }
    assert!(ReleaseScenario::parse("nope").unwrap_err().contains("nope"));
}

#[test]
fn run_mode_takes_a_profile_root_and_exactly_one_scenario() {
    use crate::webview_smoke::RunMode;
    use std::ffi::OsString;
    let parse = |args: &[&str]| RunMode::parse(args.iter().map(OsString::from));
    assert!(parse(&["--webview-release-checks"]).is_err());
    assert!(parse(&["--webview-release-checks", "/x"]).is_err());
    assert!(parse(&["--webview-release-checks", "/x", "bogus"]).is_err());
    assert_eq!(
        parse(&["--webview-release-checks", "/x", "reopen_usable"]).unwrap(),
        RunMode::WebViewReleaseChecks(PathBuf::from("/x"), ReleaseScenario::ReopenUsable)
    );
}

#[test]
fn reopen_usable_fails_by_name_when_the_start_screen_mounted() {
    let mut good = observed(false, 0, 3);
    good.workspaces.push(expectation().workspace);
    let lang = Lang::default();
    assert!(judge(ReleaseScenario::ReopenUsable, &expectation(), lang, &good).is_ok());

    let mut mounted = observed(false, 1, 3);
    mounted.workspaces.push(expectation().workspace);
    let error = judge(
        ReleaseScenario::ReopenUsable,
        &expectation(),
        lang,
        &mounted,
    )
    .unwrap_err();
    assert!(
        error.contains("reopen_usable") && error.contains("Start Screen mounted"),
        "{error}"
    );
}

#[test]
fn reopen_missing_needs_exactly_the_one_warning_toast_and_no_workspace() {
    let lang = Lang::default();
    let text = format!("{}: Gone Project", tr(lang, "workspace.reopen_failed"));
    let good = with_toast(observed(true, 1, 0), text.clone(), ToastKind::Warning);
    assert!(judge(ReleaseScenario::ReopenMissing, &expectation(), lang, &good).is_ok());

    let none = observed(true, 1, 0);
    let error = judge(ReleaseScenario::ReopenMissing, &expectation(), lang, &none).unwrap_err();
    assert!(error.contains("exactly one Warning toast"), "{error}");

    let wrong_kind = with_toast(observed(true, 1, 0), text.clone(), ToastKind::Info);
    assert!(
        judge(
            ReleaseScenario::ReopenMissing,
            &expectation(),
            lang,
            &wrong_kind
        )
        .is_err()
    );

    let two = with_toast(good, "another".into(), ToastKind::Warning);
    assert!(judge(ReleaseScenario::ReopenMissing, &expectation(), lang, &two).is_err());

    let mut fell_back = with_toast(observed(true, 1, 0), text, ToastKind::Warning);
    fell_back.workspaces.push(PathBuf::from("/ws/older"));
    let error = judge(
        ReleaseScenario::ReopenMissing,
        &expectation(),
        lang,
        &fell_back,
    )
    .unwrap_err();
    assert!(
        error.contains("a workspace opened") && error.contains("older recent entry"),
        "{error}"
    );
}

#[test]
fn reopen_disabled_needs_the_start_screen_no_workspace_and_no_toast() {
    let lang = Lang::default();
    let good = observed(true, 1, 0);
    assert!(judge(ReleaseScenario::ReopenDisabled, &expectation(), lang, &good).is_ok());

    let mut reopened = observed(false, 0, 3);
    reopened.workspaces.push(expectation().workspace);
    let error = judge(
        ReleaseScenario::ReopenDisabled,
        &expectation(),
        lang,
        &reopened,
    )
    .unwrap_err();
    assert!(
        error.contains("reopen_disabled") && error.contains("a workspace opened"),
        "{error}"
    );

    let no_screen = observed(false, 0, 0);
    let error = judge(
        ReleaseScenario::ReopenDisabled,
        &expectation(),
        lang,
        &no_screen,
    )
    .unwrap_err();
    assert!(error.contains("Start Screen is shown"), "{error}");

    let toasted = with_toast(good, "x".into(), ToastKind::Warning);
    assert!(
        judge(
            ReleaseScenario::ReopenDisabled,
            &expectation(),
            lang,
            &toasted
        )
        .is_err()
    );
}

#[test]
fn saved_bytes_must_be_the_original_plus_one_insertion() {
    let original = original_note();
    let marker = EDIT_MARKER.as_bytes();
    assert_eq!(
        check_saved_bytes(
            "save_preserves_bytes",
            &original,
            &saved_with_marker(0),
            marker
        ),
        Ok(0)
    );
    assert_eq!(
        check_saved_bytes(
            "save_preserves_bytes",
            &original,
            &saved_with_marker(3),
            marker
        ),
        Ok(3)
    );

    // Every CRLF normalised to LF: names the first differing offset.
    let normalised: Vec<u8> = String::from_utf8(saved_with_marker(0))
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes();
    let error =
        check_saved_bytes("save_preserves_bytes", &original, &normalised, marker).unwrap_err();
    assert!(error.contains("first differing byte offset 10"), "{error}");

    let mut other_byte = saved_with_marker(0);
    *other_byte.last_mut().unwrap() = b'!';
    assert!(check_saved_bytes("save_preserves_bytes", &original, &other_byte, marker).is_err());
    assert!(
        check_saved_bytes("save_preserves_bytes", &original, &original, marker)
            .unwrap_err()
            .contains("not in the saved file")
    );
    let inside_crlf = 8; // between the first line's \r and \n
    assert!(
        check_saved_bytes(
            "save_preserves_bytes",
            &original,
            &saved_with_marker(inside_crlf),
            marker
        )
        .unwrap_err()
        .contains("inside a CRLF pair")
    );
}

#[test]
fn the_seeded_note_mixes_endings_and_keeps_the_constructs() {
    let note = String::from_utf8(original_note()).unwrap();
    assert!(note.contains("\r\n") && note.contains("plain LF line\n") && !note.ends_with('\n'));
    assert!(note.contains("|---|---|") && note.contains("```rust") && note.contains("<!--"));
    assert!(!note.contains(EDIT_MARKER));
}

#[test]
fn reopen_missing_seeds_the_missing_entry_first_and_an_older_usable_one() {
    let (profile, terminal) =
        prepare(&scratch("rc-missing"), ReleaseScenario::ReopenMissing).unwrap();
    let recents = bekoedit_fs::RecentWorkspaces::load(&profile.persistence.recents_file());
    assert_eq!(recents.entries[0].display_name, "Gone Project");
    assert!(!recents.entries[0].root_path.exists(), "renamed away");
    assert!(
        recents.entries[1].root_path.exists(),
        "an older, usable entry"
    );
    assert_eq!(
        terminal.expectation.older_workspace.as_deref(),
        Some(recents.entries[1].root_path.as_path())
    );
    assert!(profile.persistence.load_settings().reopen_last_workspace);
    std::fs::remove_dir_all(&profile.root).unwrap();
}

#[test]
fn seeds_set_the_reopen_setting_and_the_file() {
    let (disabled, _) = prepare(&scratch("rc-disabled"), ReleaseScenario::ReopenDisabled).unwrap();
    assert!(!disabled.persistence.load_settings().reopen_last_workspace);
    let recents = bekoedit_fs::RecentWorkspaces::load(&disabled.persistence.recents_file());
    assert!(recents.entries[0].root_path.exists(), "usable");
    std::fs::remove_dir_all(&disabled.root).unwrap();

    let (save, terminal) =
        prepare(&scratch("rc-save"), ReleaseScenario::SavePreservesBytes).unwrap();
    let file = terminal.expectation.file.clone().unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), original_note());
    std::fs::remove_dir_all(&save.root).unwrap();
}

#[test]
fn an_unedited_file_must_be_byte_identical_and_a_change_names_its_offset() {
    let original = original_note();
    assert_eq!(
        check_bytes_unchanged("mode_switch_preserves_bytes", &original, &original),
        Ok(())
    );
    let lf_only = String::from_utf8(original.clone())
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes();
    let error =
        check_bytes_unchanged("mode_switch_preserves_bytes", &original, &lf_only).unwrap_err();
    assert!(error.contains("first differing byte offset 7"), "{error}");
    let truncated = &original[..original.len() - 1];
    assert!(
        check_bytes_unchanged("x", &original, truncated)
            .unwrap_err()
            .contains("end of file")
    );
}

#[test]
fn the_uniform_crlf_note_is_all_crlf_and_the_scenario_seeds_it() {
    let note = String::from_utf8(original_crlf_note()).unwrap();
    assert_eq!(note.matches("\r\n").count(), note.matches('\n').count());
    assert_eq!(note.matches('\r').count(), note.matches("\r\n").count());
    assert!(note.ends_with("\r\n") && !note.contains(EDIT_MARKER));
    let (profile, terminal) =
        prepare(&scratch("rc-crlf"), ReleaseScenario::SavePreservesCrlfBytes).unwrap();
    assert_eq!(terminal.expectation.original, original_crlf_note());
    assert_eq!(
        std::fs::read(terminal.expectation.file.clone().unwrap()).unwrap(),
        original_crlf_note()
    );
    std::fs::remove_dir_all(&profile.root).unwrap();
}
