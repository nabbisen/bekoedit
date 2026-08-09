//! RFC-042 (shell interaction, focus, and accessibility conformance)
//! slice guard tests. Split out of `tests.rs` (task 005) to keep that
//! file under the ELOC gate — it had grown across four slices and this
//! task's own additions pushed it past the ~450 line point prior reviews
//! flagged for splitting.

#[test]
fn rfc_042_slice_1_shell_focus_authority_is_acquired_and_restored_everywhere() {
    let shell_focus = include_str!("../shell_focus.rs");
    let app_bar = include_str!("../components/app_bar.rs");
    let header = include_str!("../components/editor_header.rs");
    let explorer = include_str!("../components/explorer.rs");
    let settings = include_str!("../components/settings_screen.rs");
    let search_panel = include_str!("../components/search_panel.rs");
    let start = include_str!("../components/start_screen.rs");
    let app = include_str!("../app.rs");
    let focus = include_str!("../source_sync/focus.rs");

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
    assert!(explorer.contains("shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH)"));
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
        search_panel.contains("shell_focus::focus_element(shell_focus::TRIGGER_WORKSPACE_SEARCH)")
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
        std::fs::read_to_string(manifest_dir.join("src/app.rs")).expect("src/app.rs is readable"),
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
    let explorer = include_str!("../components/explorer.rs");
    let tree_row = include_str!("../components/explorer/tree_row.rs");
    let tree_nav = include_str!("../components/explorer/tree_nav.rs");

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
    let shell_focus = include_str!("../shell_focus.rs");
    assert!(shell_focus.contains("pub fn focus_element(id: &'static str)"));
    assert!(shell_focus.contains("pub fn focus_tree_row(index: usize)"));

    // §6 non-change scope: the focus-authority accessors are consumed,
    // not extended — this slice adds no new method to SourceSyncState.
    let controller = include_str!("../source_sync/controller.rs");
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
    let app_bar = include_str!("../components/app_bar.rs");
    let header = include_str!("../components/editor_header.rs");
    let mode_tabs = include_str!("../components/editor_header/mode_tabs.rs");
    let shell_focus = include_str!("../shell_focus.rs");

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
    assert!(mode_tabs.contains(r#"tabindex: if mode == EditorMode::Form { "0" } else { "-1" }"#));

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
        shell_focus.contains("pub fn focus_menu_item(menu_id: &'static str, position: FocusMove)")
    );
    assert!(shell_focus.contains("pub fn focus_tab(position: FocusMove)"));
}

#[test]
fn rfc_042_slice_4_conflict_recovery_and_settings_metadata() {
    let banner = include_str!("../components/conflict_banner.rs");
    let recovery = include_str!("../components/recovery_screen.rs");
    let settings = include_str!("../components/settings_screen.rs");
    let shell_focus = include_str!("../shell_focus.rs");
    let app_bar = include_str!("../components/app_bar.rs");

    // §5.1, the most valuable assertion in this slice: the conflict
    // banner performs no focus call, in any form. It announces via
    // role="alert" and an accessible name; it never seizes focus,
    // because action 1 ("Keep my version") is destructive and the
    // banner can arrive mid-keystroke (RFC-042 §7.6, amended).
    assert!(banner.contains(r#"role: "alert""#));
    assert!(banner.contains("aria_label: tr(lang, title_key)"));
    assert!(!banner.contains("shell_focus::"));
    assert!(!banner.contains("focus_element"));
    assert!(!banner.contains(".focus()"));

    // §5.2: Recovery is a landmark region with an accessible name, a
    // live region announcing the recoverable count, and acquires/
    // releases shell focus authority on entry/exit through the
    // existing slice-1 accessors (consumed, not extended).
    assert!(recovery.contains(r#"role: "region""#));
    assert!(recovery.contains("aria_labelledby: RECOVERY_HEADING"));
    assert!(recovery.contains(r#"role: "status""#));
    assert!(recovery.contains("recovery.recoverable_suffix"));
    assert!(recovery.contains("cancel_source_focus(source_sync)"));
    assert!(recovery.contains("sync.write().release_shell_focus()"));
    assert!(recovery.contains("shell_focus::focus_element(RECOVERY_HEADING)"));
    assert!(recovery.contains("shell_focus::focus_element(shell_focus::TRIGGER_APP_LOGO)"));
    // Not trapped: no keydown handler cycling focus back inside the
    // screen, no tabindex="-1" removing any of its own controls from
    // the tab order (only the heading, which is off the tab order by
    // design — a programmatic focus target, not a Tab stop).
    assert_eq!(recovery.matches("tabindex: \"-1\"").count(), 1);

    // §5.3: Settings gains the same landmark + entry-focus treatment.
    // Its exit path (close_settings, unchanged) already acquired and
    // already releases via the slice-1 accessors — this slice adds
    // only the entry-focus move, not a second acquire.
    assert!(settings.contains(r#"role: "region""#));
    assert!(settings.contains("aria_labelledby: SETTINGS_HEADING"));
    assert!(settings.contains("shell_focus::focus_element(SETTINGS_HEADING)"));
    assert!(settings.contains("let mut close_settings"));
    assert!(settings.matches("acquire_shell_focus").count() == 0);

    // §6 non-change scope: no new eval script. Reusing the existing,
    // already balance-tested `focus_element` for all three new focus
    // moves (Recovery entry/exit, Settings entry) means this slice adds
    // zero new `document::eval` call sites — confirmed by their absence
    // from both new-surface files.
    assert!(!recovery.contains("document::eval"));
    assert!(!settings.contains("document::eval"));

    // The one new shell_focus.rs addition this slice makes: an
    // additive trigger constant, not an edit to any slice 1-3 helper.
    assert!(shell_focus.contains("pub const TRIGGER_APP_LOGO"));
    assert!(app_bar.contains("id: shell_focus::TRIGGER_APP_LOGO"));
}
