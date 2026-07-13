#[cfg(test)]
mod app_tests {
    use std::path::PathBuf;

    // ── i18n coverage: every key must exist in both EN and JA ────────────────

    #[test]
    fn i18n_all_keys_have_both_languages() {
        use crate::i18n::{Lang, tr};
        // Collect every key from EN by checking which keys return a non-empty
        // string in EN mode. Any key that exists in one language must exist in
        // both — a fallback to the key itself signals a missing translation.
        let sample_keys = [
            "app.title",
            "status.words",
            "status.chars",
            "status.islands_hint",
            "status.diag_hint",
            "save.clean",
            "save.dirty",
            "save.saving",
            "save.failed",
            "save.external_change",
            "save.conflict",
            "editor.no_document",
            "mode.text",
            "mode.form",
            "mode.preview",
            "mode.split",
            "outline.title",
            "outline.empty",
            "outline.label",
            "outline.move_up",
            "outline.move_down",
            "backlinks.title",
            "backlinks.empty",
            "backlinks.label",
            "backlinks.count_suffix",
            "history.title",
            "history.empty",
            "history.label",
            "history.restore",
            "history.restored",
            "recovery.title",
            "recovery.description",
            "recovery.restore",
            "recovery.discard",
            "recovery.skip_all",
            "recovery.restored",
            "templates.label",
            "templates.empty",
            "templates.blank",
            "island.footnote",
            "search.label",
            "search.placeholder",
            "search.empty",
            "lang.switch",
            "settings.title",
        ];
        let mut missing = Vec::new();
        for key in &sample_keys {
            let en = tr(Lang::En, key);
            let ja = tr(Lang::Ja, key);
            // tr() returns the key itself if not found.
            if en == *key {
                missing.push(format!("EN missing: {key}"));
            }
            if ja == *key {
                missing.push(format!("JA missing: {key}"));
            }
        }
        assert!(
            missing.is_empty(),
            "i18n coverage gaps:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn pending_recovery_is_detected_for_startup_screen() {
        use bekoedit_core::AppState;
        use bekoedit_fs::{RecoverySnapshot, RecoveryStore};

        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryStore::at(dir.path().join(".recovery"));
        let state = AppState::new(recovery.clone(), dir.path().join(".recent.json"), 100);
        assert!(!crate::app::has_pending_recovery(&state));

        recovery
            .save(&RecoverySnapshot {
                original_path: dir.path().join("doc.md"),
                text: "# recovered\n".into(),
                revision: 2,
                created_at_secs: 1,
            })
            .unwrap();

        assert!(crate::app::has_pending_recovery(&state));
    }

    #[test]
    fn source_command_without_active_editor_executes_now() {
        use crate::source_sync::{SourceCommand, SourceSyncState, SubmitOutcome};

        let mut sync = SourceSyncState::default();
        let command = SourceCommand::SaveNow;
        assert_eq!(
            sync.submit(command.clone(), Some(1), 100),
            SubmitOutcome::ExecuteNow(command)
        );
    }

    #[test]
    fn protected_command_from_text_waits_for_snapshot() {
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{SourceCommand, SourceEditorId, SourceSyncState, SubmitOutcome};

        let mut sync = SourceSyncState::default();
        sync.register_editor(SourceEditorId::Text, EditorMode::Text, 7, 1);

        match sync.submit(SourceCommand::SwitchMode(EditorMode::Preview), Some(7), 100) {
            SubmitOutcome::SnapshotRequested(req) => {
                assert_eq!(req.editor_id, SourceEditorId::Text);
                assert_eq!(req.document_id, 7);
                assert_eq!(req.request_id, 1);
            }
            other => panic!("expected snapshot request, got {other:?}"),
        }

        assert_eq!(
            sync.submit(SourceCommand::SaveNow, Some(7), 101),
            SubmitOutcome::Busy,
            "second protected command is single-flight blocked"
        );
    }

    #[test]
    fn accepted_snapshot_completes_pending_mode_switch() {
        use bekoedit_fs::RecoveryStore;
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{
            EditorSnapshot, SnapshotOutcome, SourceCommand, SourceEditorId, SourceSyncState,
            SubmitOutcome,
        };

        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryStore::at(dir.path().join(".recovery"));
        let mut app = bekoedit_core::AppState::new(recovery, dir.path().join(".recent.json"), 100);
        app.new_untitled();
        let doc_id = app.session.as_ref().unwrap().document_id;
        let revision = app.session.as_ref().unwrap().revision;

        let mut sync = SourceSyncState::default();
        let active = sync.register_editor(SourceEditorId::Text, EditorMode::Text, doc_id, revision);
        let request = match sync.submit(
            SourceCommand::SwitchMode(EditorMode::Preview),
            Some(doc_id),
            1,
        ) {
            SubmitOutcome::SnapshotRequested(req) => req,
            other => panic!("expected snapshot request, got {other:?}"),
        };

        let outcome = sync
            .accept_snapshot(
                &mut app,
                EditorSnapshot {
                    request_id: Some(request.request_id),
                    editor_id: SourceEditorId::Text,
                    document_id: doc_id,
                    epoch: active.epoch,
                    seq: 1,
                    text: "typed text\n".into(),
                    composing: false,
                },
                10,
            )
            .unwrap();

        assert_eq!(
            outcome,
            SnapshotOutcome::Complete(SourceCommand::SwitchMode(EditorMode::Preview))
        );
        let session = app.session.as_ref().unwrap();
        assert_eq!(session.canonical_text, "typed text\n");
        assert_eq!(session.revision, revision + 1);
        assert!(sync.pending.is_none());
    }

    #[test]
    fn noop_snapshot_completes_without_revision_bump() {
        use bekoedit_fs::RecoveryStore;
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{
            EditorSnapshot, SnapshotOutcome, SourceCommand, SourceEditorId, SourceSyncState,
            SubmitOutcome,
        };

        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryStore::at(dir.path().join(".recovery"));
        let mut app = bekoedit_core::AppState::new(recovery, dir.path().join(".recent.json"), 100);
        app.new_untitled();
        let session = app.session.as_ref().unwrap();
        let doc_id = session.document_id;
        let revision = session.revision;
        let text = session.canonical_text.clone();

        let mut sync = SourceSyncState::default();
        let active = sync.register_editor(SourceEditorId::Text, EditorMode::Text, doc_id, revision);
        let request = match sync.submit(SourceCommand::SaveNow, Some(doc_id), 1) {
            SubmitOutcome::SnapshotRequested(req) => req,
            other => panic!("expected snapshot request, got {other:?}"),
        };

        let outcome = sync
            .accept_snapshot(
                &mut app,
                EditorSnapshot {
                    request_id: Some(request.request_id),
                    editor_id: SourceEditorId::Text,
                    document_id: doc_id,
                    epoch: active.epoch,
                    seq: 1,
                    text,
                    composing: false,
                },
                10,
            )
            .unwrap();

        assert_eq!(outcome, SnapshotOutcome::Complete(SourceCommand::SaveNow));
        assert_eq!(app.session.as_ref().unwrap().revision, revision);
    }

    #[test]
    fn stale_epoch_and_request_do_not_complete_pending_command() {
        use bekoedit_fs::RecoveryStore;
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{
            EditorSnapshot, SourceCommand, SourceEditorId, SourceSyncError, SourceSyncState,
            SubmitOutcome,
        };

        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryStore::at(dir.path().join(".recovery"));
        let mut app = bekoedit_core::AppState::new(recovery, dir.path().join(".recent.json"), 100);
        app.new_untitled();
        let doc_id = app.session.as_ref().unwrap().document_id;
        let revision = app.session.as_ref().unwrap().revision;

        let mut sync = SourceSyncState::default();
        let active = sync.register_editor(SourceEditorId::Text, EditorMode::Text, doc_id, revision);
        let request = match sync.submit(SourceCommand::SaveNow, Some(doc_id), 1) {
            SubmitOutcome::SnapshotRequested(req) => req,
            other => panic!("expected snapshot request, got {other:?}"),
        };

        let stale_epoch = sync
            .accept_snapshot(
                &mut app,
                EditorSnapshot {
                    request_id: Some(request.request_id),
                    editor_id: SourceEditorId::Text,
                    document_id: doc_id,
                    epoch: active.epoch + 1,
                    seq: 1,
                    text: "typed\n".into(),
                    composing: false,
                },
                10,
            )
            .unwrap_err();
        assert_eq!(stale_epoch, SourceSyncError::EpochMismatch);
        assert!(sync.pending.is_none());

        let request = match sync.submit(SourceCommand::SaveNow, Some(doc_id), 20) {
            SubmitOutcome::SnapshotRequested(req) => req,
            other => panic!("expected snapshot request, got {other:?}"),
        };

        let stale_request = sync
            .accept_snapshot(
                &mut app,
                EditorSnapshot {
                    request_id: Some(request.request_id + 1),
                    editor_id: SourceEditorId::Text,
                    document_id: doc_id,
                    epoch: active.epoch,
                    seq: 1,
                    text: "typed\n".into(),
                    composing: false,
                },
                10,
            )
            .unwrap_err();
        assert_eq!(stale_request, SourceSyncError::RequestMismatch);
        assert!(sync.pending.is_none());
    }

    #[test]
    fn timeout_clears_pending_without_clearing_active_editor() {
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{
            SNAPSHOT_TIMEOUT_MS, SourceCommand, SourceEditorId, SourceSyncState, SubmitOutcome,
        };

        let mut sync = SourceSyncState::default();
        sync.register_editor(SourceEditorId::Text, EditorMode::Text, 7, 1);
        assert!(matches!(
            sync.submit(SourceCommand::SaveNow, Some(7), 10),
            SubmitOutcome::SnapshotRequested(_)
        ));

        assert!(sync.expire_pending(10 + SNAPSHOT_TIMEOUT_MS - 1).is_none());
        assert_eq!(
            sync.expire_pending(10 + SNAPSHOT_TIMEOUT_MS),
            Some(SourceCommand::SaveNow)
        );
        assert!(sync.pending.is_none());
        assert!(
            sync.active.is_some(),
            "Text/Split must remain mounted after timeout"
        );
    }

    #[test]
    fn expanded_protected_commands_request_snapshots() {
        use bekoedit_fs::HistoryEntry;
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{SourceCommand, SourceEditorId, SourceSyncState, SubmitOutcome};

        let history = HistoryEntry {
            original_path: PathBuf::from("/tmp/doc.md"),
            text: "old\n".into(),
            saved_at_secs: 1,
            revision: 1,
        };
        let commands = [
            SourceCommand::SaveNow,
            SourceCommand::SaveAs(PathBuf::from("/tmp/save.md")),
            SourceCommand::OpenDocument(PathBuf::from("other.md")),
            SourceCommand::NewUntitled,
            SourceCommand::OpenWorkspace(PathBuf::from("/tmp/workspace")),
            SourceCommand::CloseWorkspace,
            SourceCommand::RestoreHistory(history),
            SourceCommand::MoveSectionUp(0),
            SourceCommand::MoveSectionDown(0),
            SourceCommand::SwitchMode(EditorMode::Split),
        ];

        for command in commands {
            let mut sync = SourceSyncState::default();
            sync.register_editor(SourceEditorId::Text, EditorMode::Text, 7, 1);
            assert!(
                matches!(
                    sync.submit(command, Some(7), 100),
                    SubmitOutcome::SnapshotRequested(_)
                ),
                "protected command should request source snapshot"
            );
        }
    }

    #[test]
    fn same_document_mutations_request_editor_refresh() {
        use bekoedit_ui_contract::EditorMode;

        use crate::source_sync::{SourceEditorId, SourceSyncState};

        let mut sync = SourceSyncState::default();
        let active = sync.register_editor(SourceEditorId::Text, EditorMode::Text, 7, 2);

        sync.request_editor_refresh(SourceEditorId::Text, 7, 3);
        let refresh = sync
            .pending_refresh_for(SourceEditorId::Text)
            .expect("same-document mutation should request Text refresh");
        assert_eq!(refresh.document_id, 7);
        assert_eq!(refresh.revision, 3);
        assert!(
            refresh.epoch > active.epoch,
            "refresh must create a new editor epoch"
        );
        assert!(sync.active.as_ref().is_some_and(|editor| {
            editor.document_id == 7
                && editor.last_accepted_revision == 3
                && editor.epoch == refresh.epoch
        }));

        sync.clear_refresh(SourceEditorId::Text, refresh.epoch);
        assert!(sync.pending_refresh_for(SourceEditorId::Text).is_none());
    }
}
