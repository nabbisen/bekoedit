#[cfg(test)]
mod rfc_042;

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
        assert!(bundle.contains("armFocusGuard"));
        assert!(bundle.contains("cancelFocusGuardsThrough"));
        assert!(bundle.contains("consumeFocusGuard"));
    }

    #[test]
    fn application_root_assets_are_cargo_native_and_current() {
        let app = include_str!("app.rs");
        let host = include_str!("source_sync/host.rs");
        let placeholder = "This should be replaced by dx";

        assert!(!crate::app::STYLE_SOURCE.trim().is_empty());
        assert!(crate::app::STYLE_SOURCE.contains(".shell"));
        assert!(!crate::app::STYLE_SOURCE.contains(placeholder));

        assert!(!crate::app::SHORTCUTS_SOURCE.trim().is_empty());
        assert!(crate::app::SHORTCUTS_SOURCE.contains("window.__bk_shortcut_relay"));
        assert!(!crate::app::SHORTCUTS_SOURCE.contains(placeholder));

        assert!(!app.contains("asset!(\"/assets/style.css\")"));
        assert!(!app.contains("asset!(\"/assets/shortcuts.js\")"));
        assert!(!host.contains("asset!(\"/assets/editor-bundle.js\")"));
    }

    #[test]
    fn i18n_all_keys_have_both_languages() {
        use crate::i18n::{Lang, tr};
        let sample_keys = [
            "app.title",
            "app.tagline",
            "start.open_folder",
            "start.new_file",
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
            "editor.untitled",
            "editor.save_as",
            "mode.text",
            "mode.form",
            "mode.preview",
            "mode.split",
            "mode.close_split",
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
            "recovery.recoverable_suffix",
            "toast.dismiss",
            "table.add_row",
            "templates.label",
            "templates.empty",
            "templates.blank",
            "island.footnote",
            "search.label",
            "search.placeholder",
            "search.submit",
            "search.close",
            "search.empty",
            "explorer.cancel_new_file",
            "explorer.new_file_name",
            "explorer.create",
            "explorer.label",
            "explorer.no_workspace",
            "menu.app",
            "menu.editor_tools",
            "lang.switch",
            "settings.title",
            "settings.save_failed",
            "settings.temp_fallback_warning",
        ];
        let mut missing = Vec::new();
        for key in sample_keys {
            if tr(Lang::En, key).is_empty() {
                missing.push(format!("EN missing: {key}"));
            }
            if tr(Lang::Ja, key).is_empty() {
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
        assert!(crate::app::should_show_recovery(&state, true, false));
        assert!(!crate::app::should_show_recovery(&state, false, false));
        assert!(!crate::app::should_show_recovery(&state, true, true));

        let mut active = state;
        active.new_untitled();
        assert!(!crate::app::should_show_recovery(&active, true, false));
    }

    #[test]
    fn owner_feedback_ui_contracts_are_present() {
        let start = include_str!("components/start_screen.rs");
        let app_bar = include_str!("components/app_bar.rs");
        let header = include_str!("components/editor_header.rs");
        let form = include_str!("components/form_mode/block_view.rs");
        let toast = include_str!("components/toast.rs");
        let style = include_str!("../assets/style.css");

        assert!(start.contains("submit_source_interaction"));
        assert!(!start.contains("state.write().new_untitled()"));
        assert!(app_bar.contains("data-source-focus-launch\": \"appbar-new"));
        assert!(header.contains("data-source-focus-launch\": \"mode-split"));
        assert!(header.contains("mode.close_split"));
        assert!(!header.contains("if has_workspace"));
        assert!(header.contains("if backlinks_available"));
        assert!(header.contains("search_open.set(false)"));
        assert!(!include_str!("components/search_panel.rs").contains("search.no_results"));
        assert!(!include_str!("components/search_panel.rs").contains("search.title"));
        assert!(include_str!("components/search_panel.rs").contains("autofocus: true"));
        assert!(include_str!("components/search_panel.rs").contains("results.set(Vec::new())"));
        assert!(include_str!("components/search_panel.rs").contains("searched.set(false)"));
        assert!(include_str!("components/search_panel.rs").contains("search.close"));
        assert!(include_str!("components/explorer.rs").contains("SearchOpen"));
        assert!(include_str!("components/explorer.rs").contains("SearchPanel {}"));
        assert!(include_str!("components/explorer/tree_row.rs").contains("is_markdown_path"));
        // RFC-042 §7.4/§11 (slice 2): the native `disabled` attribute was
        // reverted — a disabled row leaves the tab order and
        // assistive-technology focus entirely, which contradicts the tree
        // pattern. Must never reappear anywhere in the row renderer.
        assert!(
            !include_str!("components/explorer/tree_row.rs").contains("disabled: !is_openable")
        );
        assert!(include_str!("components/explorer.rs").contains("workspace-new-file-name"));
        assert!(include_str!("components/explorer.rs").contains("search.label"));
        assert!(include_str!("state.rs").contains("pub enum OpenMenu"));
        assert!(app_bar.contains("stop_propagation"));
        assert!(header.contains("stop_propagation"));
        assert_eq!(
            crate::i18n::tr(crate::i18n::Lang::En, "backlinks.title"),
            "Linked from"
        );
        assert!(header.contains("class: \"adv-menu-wrap\""));
        assert!(form.contains("AddIcon {}"));
        assert!(include_str!("state.rs").contains("pub struct SettingsOpen"));
        assert!(!include_str!("app.rs").contains("use_context::<Signal<bool>>"));
        assert!(toast.contains("fn ToastItem"));
        assert!(toast.contains("toast.dismiss"));
        assert!(style.contains(".mode-tab.active"));
        assert!(style.contains("--surface: #ffffff"));
        assert!(style.contains(".adv-menu-wrap { position: relative"));
        assert!(style.contains("position: absolute; inset: 48px 8px 8px"));
        assert!(style.contains("width: min(200px, calc(100vw - 16px)); min-width: 0"));
        assert!(style.contains("width: min(180px, calc(100vw - 16px)); min-width: 0"));
        let app_menu_rule = style
            .split(".app-bar-dropdown {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .unwrap();
        let advanced_menu_rule = style
            .split(".adv-dropdown {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .unwrap();
        assert!(!app_menu_rule.contains("min-width: 200px"));
        assert!(!advanced_menu_rule.contains("min-width: 180px"));
        let used_width = |preferred: u32, viewport: u32| preferred.min(viewport.saturating_sub(16));
        assert_eq!(used_width(200, 120), 104);
        assert_eq!(used_width(180, 120), 104);
    }

    #[test]
    fn task_005_settings_layer_cleanup_contracts() {
        let fs_lib = include_str!("../../bekoedit-fs/src/lib.rs");
        let fs_settings = include_str!("../../bekoedit-fs/src/settings.rs");
        let app_settings = include_str!("settings.rs");
        let persistence = include_str!("persistence.rs");
        let settings_screen = include_str!("components/settings_screen.rs");
        let app = include_str!("app.rs");

        // Part B: the five dead persistence functions are gone; the
        // UserSettings type itself stays exported.
        assert!(!fs_settings.contains("fn default_path"));
        assert!(!fs_settings.contains("fn load("));
        assert!(!fs_settings.contains("fn save("));
        assert!(!fs_settings.contains("fn load_user_settings"));
        assert!(!fs_settings.contains("fn save_user_settings"));
        assert!(fs_lib.contains("pub use settings::UserSettings;"));

        // Part A: the fallback is a path-plus-flag return, not a bare
        // `unwrap_or_else` — the information used_temp_fallback carries no
        // longer has anywhere to be silently discarded.
        assert!(!app_settings.contains("unwrap_or_else(std::env::temp_dir)"));
        assert!(app_settings.contains("used_temp_fallback"));
        assert!(persistence.contains("fn settings_used_temp_fallback"));
        assert!(app.contains("settings_used_temp_fallback()"));
        assert!(app.contains("ToastKind::Warning"));
        assert!(app.contains("settings.temp_fallback_warning"));

        // Part C: save failures are propagated, not swallowed with `let _
        // =`, and reach the user through the toast layer.
        assert!(app_settings.contains("pub fn save(&self) -> std::io::Result<()>"));
        assert!(app_settings.contains("pub fn save_to(&self, path: &Path) -> std::io::Result<()>"));
        assert!(!app_settings.contains("let _ = bekoedit_fs::atomic_write"));
        assert!(persistence.contains(
            "pub fn save_settings(&self, settings: &AppSettings) -> std::io::Result<()>"
        ));
        assert!(settings_screen.contains("if let Err(err) = persistence.save_settings(&s)"));
        assert!(settings_screen.contains("ToastKind::Error"));
        assert!(settings_screen.contains("settings.save_failed"));

        // Re-review §2 correction: on a failed save the screen must stay
        // open, not close — `settings` is component-local, so closing
        // would unmount it, and reopening reloads the old values from
        // disk, discarding the user's edits after only the toast's
        // 4-second auto-dismiss. The failure branch returns early; the
        // success-only actions (applying the live language/mode, closing)
        // come after it, not inside it.
        let err_branch = settings_screen
            .split("if let Err(err) = persistence.save_settings(&s) {")
            .nth(1)
            .and_then(|rest| rest.split("}\n").next())
            .expect("save-button Err branch");
        assert!(err_branch.contains("return;"));
        assert!(!err_branch.contains("close_settings()"));
    }
}
