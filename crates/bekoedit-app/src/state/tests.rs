//! RFC-043 — reopen last workspace on launch.

use std::cell::RefCell;
use std::path::PathBuf;

use bekoedit_fs::{RecentWorkspaceEntry, RecentWorkspaces, RecoverySnapshot};

use super::*;
use crate::persistence::AppPersistence;

fn entry(name: &str) -> RecentWorkspaceEntry {
    RecentWorkspaceEntry {
        root_path: PathBuf::from(format!("/workspaces/{name}")),
        display_name: name.to_string(),
        last_opened_at_secs: 0,
    }
}

fn recents(entries: Vec<RecentWorkspaceEntry>) -> RecentWorkspaces {
    RecentWorkspaces { entries }
}

// ─── Pure decision — RFC-043 §10 required cases ────────────────────────────

#[test]
fn case1_disabled_setting_shows_start_screen_even_with_a_usable_recent() {
    let list = recents(vec![entry("a")]);
    let decision = decide_launch_workspace(false, &list, |_| true);
    assert_eq!(decision, LaunchWorkspaceDecision::NotAttempted);
}

#[test]
fn case2_enabled_with_no_recents_shows_start_screen() {
    let list = recents(vec![]);
    let decision = decide_launch_workspace(true, &list, |_| true);
    assert_eq!(decision, LaunchWorkspaceDecision::NotAttempted);
}

#[test]
fn case3_enabled_most_recent_usable_opens_it() {
    let list = recents(vec![entry("a")]);
    let decision = decide_launch_workspace(true, &list, |_| true);
    assert_eq!(decision, LaunchWorkspaceDecision::Opened);
}

#[test]
fn case4_enabled_most_recent_unusable_shows_start_screen_with_notice() {
    let list = recents(vec![entry("a")]);
    let decision = decide_launch_workspace(true, &list, |_| false);
    assert_eq!(
        decision,
        LaunchWorkspaceDecision::Failed {
            display_name: "a".to_string()
        }
    );
}

#[test]
fn case5_enabled_most_recent_unusable_does_not_fall_through_to_an_older_usable_entry() {
    let list = recents(vec![entry("newest-broken"), entry("older-fine")]);
    let attempts = RefCell::new(Vec::new());
    let decision = decide_launch_workspace(true, &list, |path| {
        attempts.borrow_mut().push(path.to_path_buf());
        // The OLDER entry would succeed if tried -- proving it never is
        // is the entire point of this test (RFC-043 §9).
        path == PathBuf::from("/workspaces/older-fine")
    });
    assert_eq!(
        decision,
        LaunchWorkspaceDecision::Failed {
            display_name: "newest-broken".to_string()
        },
        "must not silently open the older, usable entry"
    );
    assert_eq!(
        *attempts.borrow(),
        vec![PathBuf::from("/workspaces/newest-broken")],
        "must attempt only the most recent entry, never an older one"
    );
}

// ─── Integration — RFC-043 §10, §5 (recovery precedence) ───────────────────

fn seed_isolated_recents(root: &std::path::Path, workspace: &std::path::Path) -> AppPersistence {
    let persistence = AppPersistence::isolated(root.to_path_buf());
    let mut seeded = RecentWorkspaces::default();
    seeded.record(workspace.to_path_buf(), "workspace".to_string(), 1);
    seeded.save(&persistence.recents_file()).unwrap();
    persistence
}

#[test]
fn isolated_persistence_honours_the_enabled_reopen_setting() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let workspace = parent.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let persistence = seed_isolated_recents(&root, &workspace);

    let settings = AppSettings {
        reopen_last_workspace: true,
        ..Default::default()
    };

    let (state, notice) = create_app_state(&persistence, &settings);

    assert!(
        notice.is_none(),
        "a usable workspace must not produce a failure notice"
    );
    assert_eq!(
        state.workspace.as_ref().map(|w| w.root_path.clone()),
        Some(workspace.canonicalize().unwrap()),
        "Isolated persistence must honour the setting exactly as PlatformDefault would (handoff §3.4)"
    );
    assert!(
        state.session.is_none(),
        "must never open a document at launch (RFC-043 §5)"
    );
}

#[test]
fn isolated_persistence_disabled_setting_never_opens_a_workspace() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let workspace = parent.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let persistence = seed_isolated_recents(&root, &workspace);

    let settings = AppSettings {
        reopen_last_workspace: false,
        ..Default::default()
    };

    let (state, notice) = create_app_state(&persistence, &settings);

    assert!(notice.is_none());
    assert!(
        state.workspace.is_none(),
        "disabled setting must leave launch behaviour unchanged (acceptance criterion 2)"
    );
}

#[test]
fn isolated_persistence_unusable_recent_falls_through_to_start_screen_with_a_notice() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    // Never created on disk -- deliberately unusable.
    let missing_workspace = parent.path().join("does-not-exist");
    let persistence = seed_isolated_recents(&root, &missing_workspace);

    let settings = AppSettings {
        reopen_last_workspace: true,
        ..Default::default()
    };

    let (state, notice) = create_app_state(&persistence, &settings);

    assert!(state.workspace.is_none());
    assert_eq!(
        notice,
        Some(ReopenFailureNotice {
            display_name: "workspace".to_string()
        })
    );
    // The failing entry must remain in recents -- no prune_missing side
    // effect (handoff §3.5).
    let recents_after = RecentWorkspaces::load(&persistence.recents_file());
    assert_eq!(recents_after.entries.len(), 1);
    assert_eq!(recents_after.entries[0].root_path, missing_workspace);
}

// ─── Review finding: a successful reopen must not persist AppState's ──────
// already-pruned in-memory recents list, which would silently destroy any
// OTHER temporarily-unavailable entry (e.g. an unmounted volume) on every
// launch -- a regression this feature introduced, not pre-existing
// behaviour it surfaced.

fn seed_two_isolated_recents(
    root: &std::path::Path,
    usable: &std::path::Path,
    missing: &std::path::Path,
) -> AppPersistence {
    let persistence = AppPersistence::isolated(root.to_path_buf());
    let mut seeded = RecentWorkspaces::default();
    // record() inserts at front, so seed missing first: usable ends up
    // as entries[0], the one this feature will attempt.
    seeded.record(missing.to_path_buf(), "missing".to_string(), 1);
    seeded.record(usable.to_path_buf(), "usable-workspace".to_string(), 2);
    seeded.save(&persistence.recents_file()).unwrap();
    persistence
}

#[test]
fn a_temporarily_missing_recent_survives_a_successful_auto_reopen() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let usable = parent.path().join("usable-workspace");
    std::fs::create_dir(&usable).unwrap();
    // Never created on disk -- stands in for an unmounted volume.
    let missing = parent.path().join("does-not-exist");
    let persistence = seed_two_isolated_recents(&root, &usable, &missing);

    let settings = AppSettings {
        reopen_last_workspace: true,
        ..Default::default()
    };

    let (state, notice) = create_app_state(&persistence, &settings);
    assert!(notice.is_none());
    assert!(
        state.workspace.is_some(),
        "the usable head must have opened for this test to mean anything"
    );

    let recents_after = RecentWorkspaces::load(&persistence.recents_file());
    assert!(
        recents_after.entries.iter().any(|e| e.root_path == missing),
        "a temporarily-missing recent must survive a *successful* \
         auto-reopen, not only a failed one: {:?}",
        recents_after.entries
    );
}

#[test]
fn control_disabled_setting_never_touches_the_missing_entry() {
    // Same fixture as above, but the setting is off -- the pre-RFC-043
    // baseline. Without this control the test above proves nothing: it
    // would pass just as well if this crate never touched recents at
    // all, not because the fix actually repairs what open_workspace's
    // own save does.
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let usable = parent.path().join("usable-workspace");
    std::fs::create_dir(&usable).unwrap();
    let missing = parent.path().join("does-not-exist");
    let persistence = seed_two_isolated_recents(&root, &usable, &missing);

    let settings = AppSettings {
        reopen_last_workspace: false,
        ..Default::default()
    };

    let (state, _notice) = create_app_state(&persistence, &settings);
    assert!(state.workspace.is_none());

    let recents_after = RecentWorkspaces::load(&persistence.recents_file());
    assert!(recents_after.entries.iter().any(|e| e.root_path == missing));
}

#[test]
fn pending_recovery_still_wins_over_an_auto_opened_workspace() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let workspace = parent.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let persistence = seed_isolated_recents(&root, &workspace);

    let settings = AppSettings {
        reopen_last_workspace: true,
        ..Default::default()
    };

    let (state, _notice) = create_app_state(&persistence, &settings);
    assert!(
        state.workspace.is_some(),
        "the workspace must have opened for this test to mean anything"
    );

    state
        .recovery
        .save(&RecoverySnapshot {
            original_path: workspace.join("doc.md"),
            text: "unsaved".into(),
            revision: 1,
            created_at_secs: 1,
        })
        .unwrap();

    assert!(
        crate::app::should_show_recovery(&state, true, false),
        "recovery must still take precedence over the auto-opened workspace (RFC-043 §7)"
    );
}
