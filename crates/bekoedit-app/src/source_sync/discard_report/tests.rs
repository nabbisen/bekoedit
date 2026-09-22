//! RFC-047 slice 2 §9.1-§9.2: every command's action phrase, every reported
//! reason's clause, both languages, `Shutdown`'s silence, and the grouping
//! rule.

use std::path::PathBuf;

use bekoedit_fs::HistoryEntry;

use super::*;

fn history_entry() -> HistoryEntry {
    HistoryEntry {
        original_path: PathBuf::from("notes/a.md"),
        text: "old text".into(),
        saved_at_secs: 1,
        revision: 1,
    }
}

/// Every `SourceCommand` variant, so the exhaustiveness this module's own
/// `match` enforces is mirrored here: a variant missing from this list is a
/// gap in coverage, not a compile error, so the list is kept explicit.
fn every_command() -> Vec<SourceCommand> {
    vec![
        SourceCommand::SwitchMode(EditorMode::Text),
        SourceCommand::SwitchMode(EditorMode::Form),
        SourceCommand::SwitchMode(EditorMode::Preview),
        SourceCommand::SwitchMode(EditorMode::Split),
        SourceCommand::OpenSettings,
        SourceCommand::SaveNow,
        SourceCommand::SaveAs(PathBuf::from("sub/report.md")),
        SourceCommand::OpenDocument(PathBuf::from("sub/notes.md")),
        SourceCommand::NewUntitled,
        SourceCommand::OpenWorkspace(PathBuf::from("/workspace")),
        SourceCommand::CloseWorkspace,
        SourceCommand::RestoreHistory(history_entry()),
        SourceCommand::MoveSectionUp(2),
        SourceCommand::MoveSectionDown(2),
    ]
}

fn every_reported_reason() -> Vec<DiscardReason> {
    vec![
        DiscardReason::Overflow,
        DiscardReason::Expired,
        DiscardReason::DocumentChanged,
        DiscardReason::EditorUnavailable,
        DiscardReason::RelayLost,
    ]
}

#[test]
fn every_command_produces_a_nonempty_action_phrase_in_both_languages() {
    for command in every_command() {
        for lang in [Lang::En, Lang::Ja] {
            let phrase = action_phrase(&command, lang);
            assert!(!phrase.is_empty(), "{command:?} {lang:?}");
            assert!(
                !phrase.contains("{}"),
                "{command:?} {lang:?}: placeholder left unfilled: {phrase:?}"
            );
        }
    }
}

#[test]
fn open_document_and_save_as_name_the_file_not_the_path() {
    let open = SourceCommand::OpenDocument(PathBuf::from("sub/deep/notes.md"));
    assert_eq!(action_phrase(&open, Lang::En), "Could not open notes.md");
    assert_eq!(action_phrase(&open, Lang::Ja), "notes.mdを開けませんでした");
    assert!(!action_phrase(&open, Lang::En).contains("sub/deep"));

    let save_as = SourceCommand::SaveAs(PathBuf::from("sub/deep/report.md"));
    assert_eq!(
        action_phrase(&save_as, Lang::En),
        "Could not save as report.md"
    );
    assert_eq!(
        action_phrase(&save_as, Lang::Ja),
        "report.mdとして保存できませんでした"
    );
}

#[test]
fn open_workspace_and_restore_history_do_not_name_which_one() {
    // Handoff §3.1: these are "their own phrasing", not further identified.
    let open = SourceCommand::OpenWorkspace(PathBuf::from("/some/workspace"));
    assert_eq!(
        action_phrase(&open, Lang::En),
        "Could not open the workspace"
    );
    assert!(!action_phrase(&open, Lang::En).contains("some"));

    let restore = SourceCommand::RestoreHistory(history_entry());
    assert_eq!(
        action_phrase(&restore, Lang::En),
        "Could not restore this version"
    );
}

#[test]
fn every_reported_reason_produces_a_clause_in_both_languages() {
    for reason in every_reported_reason() {
        for lang in [Lang::En, Lang::Ja] {
            let clause = reason_clause(reason, lang)
                .unwrap_or_else(|| panic!("{reason:?} {lang:?}: expected a clause, got none"));
            assert!(!clause.is_empty(), "{reason:?} {lang:?}");
        }
    }
}

#[test]
fn document_changed_is_unmistakable_and_distinct_from_busy() {
    let en = reason_clause(DiscardReason::DocumentChanged, Lang::En).unwrap();
    assert!(en.contains("document changed"), "{en:?}");
    assert_ne!(
        en,
        reason_clause(DiscardReason::Overflow, Lang::En).unwrap()
    );
    let ja = reason_clause(DiscardReason::DocumentChanged, Lang::Ja).unwrap();
    assert!(ja.contains("ドキュメントが変更"), "{ja:?}");
}

#[test]
fn shutdown_produces_no_clause_in_either_language() {
    for lang in [Lang::En, Lang::Ja] {
        assert_eq!(reason_clause(DiscardReason::Shutdown, lang), None);
    }
}

fn discard(command: SourceCommand, reason: DiscardReason) -> QueueDiscard {
    QueueDiscard {
        command,
        reason,
        focus_token: None,
    }
}

#[test]
fn one_discard_is_one_message_naming_the_action() {
    let discards = vec![discard(
        SourceCommand::SwitchMode(EditorMode::Form),
        DiscardReason::Overflow,
    )];
    let messages = discard_messages(&discards, Lang::En);
    assert_eq!(
        messages,
        vec!["Could not switch to Form — the editor was busy.".to_string()]
    );
}

/// RFC-047 slice 2 review §3: word order is translatable data, not a Rust
/// format string one language was written to fit. English states the effect
/// then the cause; Japanese states the cause then the effect, as one
/// sentence -- not two sentences spliced with an em dash.
#[test]
fn japanese_states_the_cause_before_the_effect_as_one_sentence() {
    let discards = vec![discard(
        SourceCommand::SwitchMode(EditorMode::Form),
        DiscardReason::Overflow,
    )];
    let ja = discard_messages(&discards, Lang::Ja);
    assert_eq!(
        ja,
        vec!["エディタが使用中のため、フォームに切り替えられませんでした。".to_string()]
    );
    // No em dash, and no dangling sentence without its own closing "。".
    assert!(!ja[0].contains('—'));
    assert_eq!(ja[0].matches('。').count(), 1);
}

#[test]
fn two_discards_sharing_a_reason_report_once_naming_the_count() {
    let discards = vec![
        discard(
            SourceCommand::SwitchMode(EditorMode::Form),
            DiscardReason::Expired,
        ),
        discard(SourceCommand::SaveNow, DiscardReason::Expired),
    ];
    let messages = discard_messages(&discards, Lang::En);
    assert_eq!(
        messages,
        vec!["Could not run 2 actions — the editor was busy.".to_string()]
    );

    let ja = discard_messages(&discards, Lang::Ja);
    assert_eq!(
        ja,
        vec!["エディタが使用中のため、2件の操作を実行できませんでした。".to_string()]
    );
}

#[test]
fn two_reasons_report_twice_in_the_order_first_seen() {
    let discards = vec![
        discard(
            SourceCommand::SwitchMode(EditorMode::Form),
            DiscardReason::RelayLost,
        ),
        discard(SourceCommand::SaveNow, DiscardReason::DocumentChanged),
        discard(SourceCommand::OpenSettings, DiscardReason::RelayLost),
    ];
    let messages = discard_messages(&discards, Lang::En);
    assert_eq!(
        messages,
        vec![
            "Could not run 2 actions — the editor stopped responding.".to_string(),
            "Could not save — the document changed before it could run.".to_string(),
        ],
        "RelayLost's group (2 discards) is reported where its first discard appeared"
    );
}

#[test]
fn a_lone_discard_after_a_group_of_two_stays_singular() {
    let discards = vec![discard(
        SourceCommand::SwitchMode(EditorMode::Preview),
        DiscardReason::Overflow,
    )];
    let messages = discard_messages(&discards, Lang::En);
    assert_eq!(
        messages,
        vec!["Could not switch to Preview — the editor was busy.".to_string()]
    );
    assert!(
        !messages[0].contains("actions"),
        "a single discard never says \"actions\""
    );
}

#[test]
fn shutdown_discards_produce_no_message_and_no_empty_group() {
    let discards = vec![
        discard(
            SourceCommand::SwitchMode(EditorMode::Form),
            DiscardReason::Shutdown,
        ),
        discard(SourceCommand::SaveNow, DiscardReason::Shutdown),
    ];
    assert!(discard_messages(&discards, Lang::En).is_empty());
    assert!(discard_messages(&discards, Lang::Ja).is_empty());
}

#[test]
fn a_shutdown_discard_mixed_with_a_reported_one_only_reports_the_latter() {
    let discards = vec![
        discard(
            SourceCommand::SwitchMode(EditorMode::Form),
            DiscardReason::Shutdown,
        ),
        discard(SourceCommand::SaveNow, DiscardReason::Overflow),
    ];
    let messages = discard_messages(&discards, Lang::En);
    assert_eq!(
        messages,
        vec!["Could not save — the editor was busy.".to_string()]
    );
}
