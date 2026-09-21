use super::*;
use crate::webview_smoke::transport::{MessageKind, PhaseMessage, SMOKE_PROTOCOL_VERSION};

fn phase_message(kind: MessageKind, phase: &str, exchange_id: u64) -> PhaseMessage {
    PhaseMessage {
        protocol_version: SMOKE_PROTOCOL_VERSION,
        exchange_id,
        kind,
        phase: phase.into(),
        released_exchange_id: None,
        released_phase: None,
        milestone: None,
        result: None,
    }
}

fn successful_result() -> DriverResult {
    DriverResult {
        ok: true,
        stage: TERMINAL_STAGE.into(),
        marker: MARKER.into(),
        milestones: EXPECTED_MILESTONES
            .iter()
            .map(|item| (*item).into())
            .collect(),
        error_toast_seen: false,
        error: None,
    }
}

#[test]
fn machine_advances_through_every_transition_ending_at_conflict_banner() {
    let mut machine = ShellBehaviourMachine::new();
    let progression = [
        (
            ShellBehaviourPhase::RecoveryEntry,
            "recovery_heading_focused",
            ShellBehaviourPhase::RecoveryExit,
        ),
        (
            ShellBehaviourPhase::RecoveryExit,
            "recovery_exit_restored_logo",
            ShellBehaviourPhase::DownUp,
        ),
        (
            ShellBehaviourPhase::DownUp,
            "down_up_moved",
            ShellBehaviourPhase::ExpandEnter,
        ),
        (
            ShellBehaviourPhase::ExpandEnter,
            "expand_entered",
            ShellBehaviourPhase::CollapseAscend,
        ),
        (
            ShellBehaviourPhase::CollapseAscend,
            "collapse_ascended",
            ShellBehaviourPhase::HomeEnd,
        ),
        (
            ShellBehaviourPhase::HomeEnd,
            "home_end_reached",
            ShellBehaviourPhase::NonOpenable,
        ),
        (
            ShellBehaviourPhase::NonOpenable,
            "non_openable_reachable",
            ShellBehaviourPhase::EnterOpens,
        ),
        (
            ShellBehaviourPhase::EnterOpens,
            "enter_opened_editor_focused",
            ShellBehaviourPhase::SearchResultOpens,
        ),
        (
            ShellBehaviourPhase::SearchResultOpens,
            "search_result_editor_focused",
            ShellBehaviourPhase::NewFileFocuses,
        ),
        (
            ShellBehaviourPhase::NewFileFocuses,
            "new_file_editor_focused",
            ShellBehaviourPhase::TreeEnterAfterNewFile,
        ),
        (
            ShellBehaviourPhase::TreeEnterAfterNewFile,
            "tree_enter_refocused_after_new_file",
            ShellBehaviourPhase::FormSearchRestores,
        ),
        (
            ShellBehaviourPhase::FormSearchRestores,
            "form_search_restored_to_trigger",
            ShellBehaviourPhase::AppMenuMouseOpen,
        ),
        (
            ShellBehaviourPhase::AppMenuMouseOpen,
            "app_menu_mouse_open_kept_focus",
            ShellBehaviourPhase::AppMenuKeys,
        ),
        (
            ShellBehaviourPhase::AppMenuKeys,
            "app_menu_keys_verified",
            ShellBehaviourPhase::AppMenuEscape,
        ),
        (
            ShellBehaviourPhase::AppMenuEscape,
            "app_menu_escape_restored",
            ShellBehaviourPhase::AppMenuFocusLeave,
        ),
        (
            ShellBehaviourPhase::AppMenuFocusLeave,
            "app_menu_focus_leave_kept",
            ShellBehaviourPhase::ToolsMenuKeys,
        ),
        (
            ShellBehaviourPhase::ToolsMenuKeys,
            "tools_menu_keys_verified",
            ShellBehaviourPhase::ToolsMenuEscape,
        ),
        (
            ShellBehaviourPhase::ToolsMenuEscape,
            "tools_menu_escape_restored",
            ShellBehaviourPhase::ToolsMenuFocusLeave,
        ),
        (
            ShellBehaviourPhase::ToolsMenuFocusLeave,
            "tools_menu_focus_leave_kept",
            ShellBehaviourPhase::TabsArrowsFocusOnly,
        ),
        (
            ShellBehaviourPhase::TabsArrowsFocusOnly,
            "tabs_arrows_moved_focus_only",
            ShellBehaviourPhase::TabsClickActivates,
        ),
        (
            ShellBehaviourPhase::TabsClickActivates,
            "tabs_click_focused_editor",
            ShellBehaviourPhase::MenuClosesIntoEditor,
        ),
        (
            ShellBehaviourPhase::MenuClosesIntoEditor,
            "menu_closed_into_editor_kept",
            ShellBehaviourPhase::AuthorityReleasedAfterEditorFocus,
        ),
        (
            ShellBehaviourPhase::AuthorityReleasedAfterEditorFocus,
            "authority_released_editor_refocused",
            ShellBehaviourPhase::SettingsEntry,
        ),
        (
            ShellBehaviourPhase::SettingsEntry,
            "settings_heading_focused",
            ShellBehaviourPhase::SettingsExitRestored,
        ),
        (
            ShellBehaviourPhase::SettingsExitRestored,
            "settings_exit_restored_trigger",
            ShellBehaviourPhase::ConflictDirtied,
        ),
        (
            ShellBehaviourPhase::ConflictDirtied,
            "conflict_document_dirtied",
            ShellBehaviourPhase::ConflictBannerFocusKept,
        ),
    ];
    for (index, (phase, milestone, next)) in progression.into_iter().enumerate() {
        let exchange_id = (index + 1) as u64;
        assert_eq!(machine.current(), phase);
        let mut message = phase_message(MessageKind::Progress, phase.as_str(), exchange_id);
        message.milestone = Some(milestone.into());
        machine.validate(&message, exchange_id, None).unwrap();
        machine.apply_completed(exchange_id, &message).unwrap();
        assert_eq!(machine.current(), next);
    }
    assert_eq!(
        ShellBehaviourPhase::ConflictBannerFocusKept.next(),
        None,
        "conflict_banner_focus_kept (slice 3, F2) is the terminal phase"
    );
    assert_eq!(
        ShellBehaviourPhase::ConflictBannerFocusKept.as_str(),
        TERMINAL_STAGE
    );
}

#[test]
fn terminal_can_come_from_any_phase_not_only_the_last_one() {
    // Regression test: a driver's try/catch turns a thrown error into a
    // terminal *failure* at whichever phase raised it -- exactly as
    // driver.js's own three phases each can. An earlier version of this
    // validate() incorrectly restricted Terminal to the last phase only,
    // which meant a real down_up failure surfaced as "only enter_opens can
    // return a terminal result" instead of the driver's actual error
    // (caught by CI on the first real run against a WebView, 2026-09-04).
    let machine = ShellBehaviourMachine::new();
    assert_eq!(machine.current(), ShellBehaviourPhase::RecoveryEntry);
    let mut message = phase_message(MessageKind::Terminal, "recovery_entry", 1);
    let mut failed = successful_result();
    failed.ok = false;
    failed.stage = "recovery_entry".into();
    failed.error = Some("could not focus the first tree row directly".into());
    message.result = Some(failed);
    machine.validate(&message, 1, None).unwrap();
}

#[test]
fn malformed_progress_and_terminal_messages_are_rejected() {
    let machine = ShellBehaviourMachine::for_phase(ShellBehaviourPhase::DownUp);

    let mut wrong_milestone = phase_message(MessageKind::Progress, "down_up", 1);
    wrong_milestone.milestone = Some("expand_entered".into());
    assert!(machine.validate(&wrong_milestone, 1, None).is_err());

    let progress_with_result = {
        let mut message = phase_message(MessageKind::Progress, "down_up", 1);
        message.milestone = Some("down_up_moved".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&progress_with_result, 1, None).is_err());

    let terminal_with_milestone = {
        let mut message = phase_message(MessageKind::Terminal, "down_up", 1);
        message.milestone = Some("down_up_moved".into());
        message.result = Some(successful_result());
        message
    };
    assert!(machine.validate(&terminal_with_milestone, 1, None).is_err());

    let terminal_without_result = phase_message(MessageKind::Terminal, "down_up", 1);
    assert!(machine.validate(&terminal_without_result, 1, None).is_err());

    let out_of_order = phase_message(MessageKind::Pending, "expand_enter", 1);
    assert!(machine.validate(&out_of_order, 1, None).is_err());

    let last_phase_terminal_progress =
        ShellBehaviourMachine::for_phase(ShellBehaviourPhase::ConflictBannerFocusKept);
    let mut malformed = phase_message(MessageKind::Progress, TERMINAL_STAGE, 1);
    malformed.milestone = Some("conflict_banner_focus_kept".into());
    assert!(
        last_phase_terminal_progress
            .validate(&malformed, 1, None)
            .is_err(),
        "the terminal phase cannot return nonterminal progress"
    );

    // enter_opens is no longer terminal (task 016): its progress is valid.
    let enter_opens = ShellBehaviourMachine::for_phase(ShellBehaviourPhase::EnterOpens);
    let mut progress = phase_message(MessageKind::Progress, "enter_opens", 1);
    progress.milestone = Some("enter_opened_editor_focused".into());
    assert!(enter_opens.validate(&progress, 1, None).is_ok());
}

#[test]
fn validate_result_checks_stage_marker_toast_and_milestones() {
    assert!(validate_shell_behaviour_result(&successful_result()).is_ok());

    let mutations: [fn(&mut DriverResult); 5] = [
        |result| result.stage = "enter_opens".into(),
        |result| result.marker = "wrong".into(),
        |result| result.error_toast_seen = true,
        |result| result.error = Some("contradictory success".into()),
        |result| {
            result.milestones.swap(0, 1);
        },
    ];
    for mutate in mutations {
        let mut result = successful_result();
        mutate(&mut result);
        assert!(validate_shell_behaviour_result(&result).is_err());
    }

    let mut failed = successful_result();
    failed.ok = false;
    failed.error = Some("explicit failure".into());
    assert!(validate_shell_behaviour_result(&failed).is_err());
}

#[test]
fn terminal_result_transitions_the_terminal_exactly_once() {
    let terminal = ShellBehaviourTerminal::default();
    assert!(!terminal.succeeded());
    assert!(terminal.accept(&successful_result()).is_ok());
    assert!(terminal.succeeded());
    assert!(
        terminal.accept(&successful_result()).is_err(),
        "duplicates are rejected"
    );
    assert!(terminal.succeeded(), "late results cannot reverse success");
}

#[test]
fn the_conflict_write_happens_only_between_f1_and_f2() {
    // Slice 3 §4.2: after F1's progress and before F2 is requested -- and
    // after no other phase, so no earlier phase sees a changed file.
    let mut phase = Some(ShellBehaviourMachine::new().current());
    let mut writers = Vec::new();
    while let Some(current) = phase {
        if writes_conflict_after(current) {
            writers.push(current);
        }
        phase = current.next();
    }
    assert_eq!(writers, [ShellBehaviourPhase::ConflictDirtied]);
    assert_eq!(
        ShellBehaviourPhase::ConflictDirtied.next(),
        Some(ShellBehaviourPhase::ConflictBannerFocusKept)
    );
}

fn scratch_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bekoedit-webview-smoke-test-{label}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn the_conflict_write_changes_the_length_and_names_the_path_on_failure() {
    let dir = scratch_dir("write");
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("child.md");
    std::fs::write(&file, "# child\n").unwrap();

    write_conflicting_change(&file).unwrap();
    let changed = std::fs::read_to_string(&file).unwrap();
    assert!(changed.starts_with("# child\n"));
    assert_ne!(
        changed.len(),
        "# child\n".len(),
        "detection needs a length change"
    );

    let missing = dir.join("absent").join("child.md");
    let error = write_conflicting_change(&missing).unwrap_err();
    assert!(error.contains(&missing.display().to_string()), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prepare_seeds_one_recovery_snapshot_a_one_day_debounce_and_the_conflict_file() {
    let root = scratch_dir("prepare");
    let prepared = prepare(&root).unwrap();
    let paths = prepared
        .persistence
        .isolated_paths()
        .expect("isolated persistence");

    let snapshots = RecoveryStore::at(paths.recovery_dir().to_path_buf()).list();
    assert_eq!(snapshots.len(), 1, "exactly one snapshot, so E1 reads `1 `");
    let workspace = paths.root().join("workspace");
    assert_eq!(snapshots[0].original_path, workspace.join("a.md"));
    let on_disk = std::fs::read_to_string(workspace.join("a.md")).unwrap();
    assert_ne!(snapshots[0].text, on_disk);

    let settings = prepared.persistence.load_settings();
    assert_eq!(
        settings.core.autosave_debounce_ms,
        CONFLICT_AUTOSAVE_DEBOUNCE_MS
    );
    assert_eq!(
        CONFLICT_AUTOSAVE_DEBOUNCE_MS, 86_400_000,
        "one day, not u64::MAX"
    );

    assert_eq!(
        prepared.conflict_file,
        workspace.join("sub").join("child.md")
    );
    assert!(prepared.conflict_file.is_file());
    std::fs::remove_dir_all(&prepared.root).unwrap();
}

mod settle_gate;
