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
    fn rfc_042_slice_1_shell_focus_authority_is_acquired_and_restored_everywhere() {
        let shell_focus = include_str!("shell_focus.rs");
        let app_bar = include_str!("components/app_bar.rs");
        let header = include_str!("components/editor_header.rs");
        let explorer = include_str!("components/explorer.rs");
        let settings = include_str!("components/settings_screen.rs");
        let search_panel = include_str!("components/search_panel.rs");
        let start = include_str!("components/start_screen.rs");
        let app = include_str!("app.rs");
        let focus = include_str!("source_sync/focus.rs");

        // shell_focus.rs exposes the four stable trigger ids and a single
        // centralised restore primitive — no scattered eval strings. The id
        // parameter is `&'static str`, not `&str` (review R5): slice 2's
        // user-controlled row ids must never reach it unsanitized.
        assert!(shell_focus.contains("pub const TRIGGER_APP_MENU"));
        assert!(shell_focus.contains("pub const TRIGGER_EDITOR_TOOLS"));
        assert!(shell_focus.contains("pub const TRIGGER_WORKSPACE_SEARCH"));
        assert!(shell_focus.contains("pub const TRIGGER_NEW_FILE"));
        assert!(shell_focus.contains("pub fn focus_element(id: &'static str)"));

        // cancel_source_focus routes through acquire_shell_focus (handoff §7.3);
        // cancel_pending_source_focus exists as the non-acquiring counterpart
        // for native-dialog call sites (re-review C1/C2).
        assert!(focus.contains("sync.write().acquire_shell_focus()"));
        assert!(focus.contains("pub fn cancel_pending_source_focus"));

        // Native-dialog call sites acquire no authority they cannot release
        // (re-review C1, C2): Start Screen's Open Folder and Save As use the
        // non-acquiring cancel, not cancel_source_focus.
        assert!(start.contains("cancel_pending_source_focus(source_sync)"));
        assert!(header.contains("cancel_pending_source_focus(source_sync)"));

        // App menu trigger: stable id, acquires on open, releases/restores on
        // explicit close. The home-logo click releases WITHOUT restoring
        // (implicit dismissal, re-review C3) — never
        // release_and_restore_menu_focus there.
        assert!(app_bar.contains("id: shell_focus::TRIGGER_APP_MENU"));
        assert!(app_bar.contains("cancel_source_focus(source_sync)"));
        assert!(app_bar.contains("fn release_menu_focus"));
        assert!(app_bar.contains("fn release_and_restore_menu_focus"));
        assert!(app_bar.contains("release_shell_focus()"));
        assert!(app_bar.contains("shell_focus::focus_element(trigger)"));
        let logo_onclick = app_bar
            .split("class: \"app-bar-logo\"")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("app-bar-logo button body");
        assert!(logo_onclick.contains("release_menu_focus(source_sync"));
        assert!(!logo_onclick.contains("release_and_restore_menu_focus"));

        // Editor tools trigger: same contract.
        assert!(header.contains("id: shell_focus::TRIGGER_EDITOR_TOOLS"));
        assert!(header.contains("close_editor_tools_menu"));
        assert!(header.contains("release_shell_focus()"));
        assert!(header.contains("shell_focus::focus_element(shell_focus::TRIGGER_EDITOR_TOOLS)"));

        // Explorer: workspace-search trigger and new-file disclosure trigger.
        assert!(explorer.contains("id: shell_focus::TRIGGER_WORKSPACE_SEARCH"));
        assert!(
            explorer.contains("shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH)")
        );
        assert!(explorer.contains("id: shell_focus::TRIGGER_NEW_FILE"));
        assert!(explorer.contains("shell_focus::focus_element(shell_focus::TRIGGER_NEW_FILE)"));

        // M4: shell surfaces stay mutually exclusive — opening either menu
        // also closes the workspace-search disclosure, not just the reverse.
        assert!(app_bar.contains("RFC-042 M4"));
        assert!(header.contains("RFC-042 M4"));

        // Settings screen replacement: acquires on entry, releases/restores on exit.
        assert!(app_bar.contains("SourceCommand::OpenSettings"));
        assert!(settings.contains("close_settings"));
        assert!(settings.contains("release_shell_focus()"));
        assert!(settings.contains("shell_focus::focus_element(shell_focus::TRIGGER_APP_MENU)"));

        // Search panel's own close paths (×, Escape, result click) release too.
        assert!(search_panel.contains("close_search"));
        assert!(search_panel.contains("release_shell_focus()"));
        assert!(
            search_panel
                .contains("shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH)")
        );

        // Outside-click / focus-leave (app.rs) releases WITHOUT restoring
        // (implicit dismissal, re-review C3) — must not fight focus the
        // RFC-041 controller just placed in CodeMirror.
        assert!(app.contains("release_menu_focus"));
        assert!(!app.contains("release_and_restore_menu_focus"));
    }

    #[test]
    fn rfc_042_every_shell_focus_acquire_has_a_release_in_the_same_file() {
        // Re-review correction C-ALL, then K2. `cancel_source_focus` claims
        // shell focus authority; every file that calls it must also release
        // that authority somewhere in the same file — or be named here with
        // the reason it doesn't (e.g. authority is handed to a different
        // screen that owns the close path). This is intentionally coarse —
        // a per-file existence check, not a per-call-site pairing proof —
        // but it turns "the next person must remember" into a failing test.
        //
        // K2: the file list is enumerated from disk at runtime, not
        // hardcoded, so a file this pass never touched — e.g. a future
        // slice's `conflict_banner.rs` or `recovery_screen.rs` — is covered
        // automatically the moment it starts calling `cancel_source_focus`.
        //
        // Settings is the one legitimate hand-off: app_bar.rs's Settings
        // menu item acquires, but settings_screen.rs releases, because the
        // menu item itself unmounts when the dropdown closes. app_bar.rs
        // still passes this check unaided — it also releases for its own
        // menu's other close paths — so no allow-list entry is needed.
        const ALLOW_LISTED_NO_RELEASE_IN_FILE: &[&str] = &[];

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = Vec::new();
        collect_rust_sources(
            &manifest_dir.join("src/components"),
            "components",
            &mut checked,
        );
        checked.push((
            "app.rs".to_string(),
            std::fs::read_to_string(manifest_dir.join("src/app.rs"))
                .expect("src/app.rs is readable"),
        ));

        // Sanity: the walk actually found the known acquiring files, so a
        // broken or empty directory read can't silently pass this test by
        // finding nothing to check.
        assert!(
            checked
                .iter()
                .any(|(path, _)| path == "components/app_bar.rs")
        );
        assert!(
            checked
                .iter()
                .any(|(path, _)| path == "components/editor_header.rs")
        );
        assert!(
            checked.len() >= 6,
            "expected at least 6 files, found {}",
            checked.len()
        );

        for (path, source) in &checked {
            let acquires = source.contains("cancel_source_focus(");
            let releases = source.contains("release_shell_focus()");
            if acquires && !releases {
                assert!(
                    ALLOW_LISTED_NO_RELEASE_IN_FILE.contains(&path.as_str()),
                    "{path} calls cancel_source_focus (acquires shell focus authority) \
                     but never calls release_shell_focus in the same file, and is not \
                     on the allow-list. Either add a release path or document why \
                     authority is released elsewhere and add it to the allow-list.",
                );
            }
        }
    }

    /// Recursively collects `(relative_path, source)` for every `.rs` file
    /// under `dir`, labeling each with `label` (e.g. `"components"`) so the
    /// relative path matches what a reader sees in the repository.
    fn collect_rust_sources(dir: &std::path::Path, label: &str, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("read_dir({}): {error}", dir.display()));
        for entry in entries {
            let entry = entry.expect("directory entry is readable");
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                collect_rust_sources(&path, &format!("{label}/{file_name}"), out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            out.push((format!("{label}/{file_name}"), source));
        }
    }

    #[test]
    fn rfc_042_slice_2_tree_conforms_to_the_wai_aria_tree_pattern() {
        let explorer = include_str!("components/explorer.rs");
        let tree_row = include_str!("components/explorer/tree_row.rs");
        let tree_nav = include_str!("components/explorer/tree_nav.rs");

        // Roving tabindex: exactly one row in the tab order at a time, never
        // every row (§7.2, §11).
        assert!(tree_row.contains(r#"tabindex: if is_active { "0" } else { "-1" }"#));

        // Active vs selected are two different, both-required things (§7.3).
        assert!(tree_row.contains("is_active: bool"));
        assert!(tree_row.contains("is_selected: bool"));
        assert!(tree_row.contains("aria_selected"));

        // §7.4: the native `disabled` attribute was reverted — a
        // non-openable row stays focusable and announced via
        // `aria-disabled`, not removed from the tab order.
        assert!(tree_row.contains("aria_disabled"));
        assert!(!tree_row.contains("disabled: !is_openable"));
        assert!(!explorer.contains("disabled: !is_openable"));

        // §7.1: navigation logic lives in the pure module, not inline in the
        // RSX closure (§11 prohibited shortcuts).
        assert!(tree_row.contains("tree_nav::navigate"));
        assert!(tree_nav.contains("pub fn navigate"));

        // §12: only an index reaches the eval'd script — no per-row id, no
        // path-derived string, and `focus_element`/the trigger constants are
        // untouched by this slice.
        assert!(tree_row.contains("shell_focus::focus_tree_row"));
        assert!(!tree_row.contains("shell_focus::focus_element"));
        let shell_focus = include_str!("shell_focus.rs");
        assert!(shell_focus.contains("pub fn focus_element(id: &'static str)"));
        assert!(shell_focus.contains("pub fn focus_tree_row(index: usize)"));

        // §6 non-change scope: the focus-authority accessors are consumed,
        // not extended — this slice adds no new method to SourceSyncState.
        let controller = include_str!("source_sync/controller.rs");
        assert!(controller.contains("pub fn acquire_shell_focus"));
        assert!(controller.contains("pub fn release_shell_focus"));
        assert!(controller.contains("pub fn shell_focus_held"));

        // DEC-011: no drag path reintroduced.
        assert!(!explorer.contains("DirectoryTreeView"));
        assert!(!tree_row.contains("DirectoryTreeView"));
        assert!(!explorer.contains("DragState"));
        assert!(!tree_row.contains("DragState"));
    }

    #[test]
    fn rfc_042_slice_3_menu_and_tab_keyboard_contracts() {
        let app_bar = include_str!("components/app_bar.rs");
        let header = include_str!("components/editor_header.rs");
        let mode_tabs = include_str!("components/editor_header/mode_tabs.rs");
        let shell_focus = include_str!("shell_focus.rs");

        // §5.1: menu items are not tab stops — every item carries
        // tabindex="-1". app_bar.rs: 4 items (Open Folder, New File, Close
        // Workspace, Settings) plus the pre-existing container-level
        // tabindex="-1" on the dropdown itself = 5. editor_header.rs: 5
        // items (Split, Outline, Backlinks, History, Export), no container
        // tabindex there.
        assert_eq!(app_bar.matches("tabindex: \"-1\"").count(), 5);
        assert_eq!(header.matches("tabindex: \"-1\"").count(), 5);

        // §5.5: exactly one tab's tabindex is bound to *selection*
        // (`mode == m` / `mode == EditorMode::Form`), not to a separately
        // tracked "last focused" signal — this is what keeps tabbing into
        // the tablist always landing on the current mode.
        assert!(mode_tabs.contains(r#"tabindex: if mode == m { "0" } else { "-1" }"#));
        assert!(
            mode_tabs.contains(r#"tabindex: if mode == EditorMode::Form { "0" } else { "-1" }"#)
        );

        // §5.4 / C3 regression guard, the most valuable assertion in this
        // set: Escape is explicit dismissal (routes through the
        // release-*and-restore* helper via close_*_menu); Tab is implicit
        // dismissal and must NOT be intercepted here at all — no
        // competing handler, relying on app.rs's existing onfocusin route
        // (release_menu_focus, no restore) once items stop being tab
        // stops.
        assert!(app_bar.contains("Key::Escape") && app_bar.contains("close_app_menu()"));
        assert!(header.contains("Key::Escape") && header.contains("close_editor_tools_menu()"));
        assert!(!app_bar.contains("Key::Tab"));
        assert!(!header.contains("Key::Tab"));

        // §5.3: item resolution is DOM-relative — no Rust-side mirror of
        // the menu item list. `focus_menu_item` reads
        // `[role="menuitem"]` from the DOM at the moment of use.
        assert!(shell_focus.contains("querySelectorAll('[role=\"menuitem\"]')"));
        assert!(shell_focus.contains("querySelectorAll('[role=\"tab\"]')"));

        // §5.6: arrow-navigating tabs must not disturb source focus. The
        // tablist's keydown handler calls only `shell_focus::tab_key_intent`
        // and `shell_focus::focus_tab` — never anything from `source_sync`.
        let onkeydown = mode_tabs
            .split("onkeydown: move |event: KeyboardEvent| {")
            .nth(1)
            .and_then(|rest| rest.split("},").next())
            .expect("mode-tabs onkeydown body");
        assert!(!onkeydown.contains("source_sync"));
        assert!(!onkeydown.contains("cancel_source_focus"));
        assert!(!onkeydown.contains("acquire_shell_focus"));
        assert!(!onkeydown.contains("submit_source"));

        // §6 non-change scope: focus_element/focus_tree_row and the four
        // trigger constants are untouched by this slice.
        assert!(shell_focus.contains("pub fn focus_element(id: &'static str)"));
        assert!(shell_focus.contains("pub fn focus_tree_row(index: usize)"));
        assert!(shell_focus.contains("pub const TRIGGER_APP_MENU"));
        assert!(shell_focus.contains("pub const TRIGGER_EDITOR_TOOLS"));
        assert!(shell_focus.contains("pub const TRIGGER_WORKSPACE_SEARCH"));
        assert!(shell_focus.contains("pub const TRIGGER_NEW_FILE"));

        // §9.5: only &'static str and integers reach any eval'd script —
        // the new focus movers take the same shape as slice 1/2's.
        assert!(
            shell_focus
                .contains("pub fn focus_menu_item(menu_id: &'static str, position: FocusMove)")
        );
        assert!(shell_focus.contains("pub fn focus_tab(position: FocusMove)"));
    }
}
