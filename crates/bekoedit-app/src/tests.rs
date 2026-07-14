#[cfg(test)]
mod app_tests {
    #[test]
    fn rust_and_javascript_bridge_versions_match() {
        let lifecycle = include_str!("../js/src/lifecycle.js");
        let editor = include_str!("../js/src/editor.js");
        let bundle = include_str!("../assets/editor-bundle.js");
        assert_eq!(bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION, 2);
        assert!(lifecycle.contains("BRIDGE_SCHEMA_VERSION = 2"));
        assert!(editor.contains("export { BRIDGE_SCHEMA_VERSION"));
        assert!(bundle.contains("protocolVersion:2"));
        assert!(bundle.contains("window.__bk="));
    }

    #[test]
    fn i18n_all_keys_have_both_languages() {
        use crate::i18n::{Lang, tr};
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
            "editor.loading",
            "editor.unavailable",
            "editor.retry",
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
        for key in sample_keys {
            if tr(Lang::En, key) == key {
                missing.push(format!("EN missing: {key}"));
            }
            if tr(Lang::Ja, key) == key {
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
}
