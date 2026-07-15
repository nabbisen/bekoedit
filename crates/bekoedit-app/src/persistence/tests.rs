use bekoedit_fs::RecoverySnapshot;

use super::*;

#[test]
fn isolated_persistence_routes_every_store_below_its_root() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("isolated");
    std::fs::create_dir(&root).unwrap();
    let persistence = AppPersistence::isolated(root.canonicalize().unwrap());
    let paths = persistence.isolated_paths().unwrap();
    assert!(paths.all_within_root());

    let settings = AppSettings::default();
    persistence.save_settings(&settings);
    assert!(paths.settings_file().exists());

    let workspace = parent.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("doc.md"), "# before\n").unwrap();
    let mut state = persistence.create_app_state(100);
    state.open_workspace(&workspace, 1).unwrap();
    state.open_document(Path::new("doc.md")).unwrap();
    let revision = state.session.as_ref().unwrap().revision;
    state
        .edit_text(revision, "# after\n".into(), 2_000)
        .unwrap();
    state.save_now(3_000).unwrap();

    state
        .recovery
        .save(&RecoverySnapshot {
            original_path: workspace.join("recovery.md"),
            text: "unsaved".into(),
            revision: 1,
            created_at_secs: 1,
        })
        .unwrap();

    assert!(paths.recents_file().exists());
    assert!(paths.recovery_dir().read_dir().unwrap().next().is_some());
    assert!(paths.history_dir().read_dir().unwrap().next().is_some());
    for path in [
        paths.settings_file(),
        paths.recents_file(),
        paths.recovery_dir(),
        paths.history_dir(),
    ] {
        assert!(path.starts_with(paths.root()));
    }
}

#[test]
fn platform_default_variant_has_no_isolated_paths() {
    assert!(
        AppPersistence::platform_default()
            .isolated_paths()
            .is_none()
    );
}
