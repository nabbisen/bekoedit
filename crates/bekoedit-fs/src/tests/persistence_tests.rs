// Atomic save, fingerprints, recovery, recent workspaces, and settings.

use crate::atomic::{FileFingerprint, atomic_write};
use crate::recent::RecentWorkspaces;
use crate::recovery::{RecoverySnapshot, RecoveryStore};
use crate::{UserSettings, load_user_settings, save_user_settings};

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// --- atomic save + fingerprints (RFC-007 / RFC-008) ---

#[test]
fn atomic_write_round_trips_and_fingerprints_detect_change() {
    let dir = temp_workspace();
    let file = dir.path().join("doc.md");
    let fp = atomic_write(&file, "# v1\n").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "# v1\n");
    assert!(!fp.disk_changed(&file).unwrap());

    std::fs::write(&file, "# external\n").unwrap();
    assert!(fp.disk_changed(&file).unwrap());

    let fp2 = FileFingerprint::read(&file).unwrap();
    assert!(!fp2.disk_changed(&file).unwrap());
}

#[test]
fn atomic_write_leaves_no_temp_files() {
    let dir = temp_workspace();
    atomic_write(&dir.path().join("a.md"), "x").unwrap();
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
    assert_eq!(entries.len(), 1, "only the final file should exist");
}

// --- recovery (RFC-007) ---

#[test]
fn recovery_snapshots_persist_and_clear() {
    let dir = temp_workspace();
    let store = RecoveryStore::at(dir.path().join("recovery"));
    let snap = RecoverySnapshot {
        original_path: dir.path().join("doc.md"),
        text: "# Recovered\n".into(),
        revision: 7,
        created_at_secs: 1000,
    };
    store.save(&snap).unwrap();
    let listed = store.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].revision, 7);
    store.remove(&snap.original_path).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn recovery_returns_empty_when_dir_absent() {
    let dir = temp_workspace();
    let store = RecoveryStore::at(dir.path().join("nonexistent"));
    assert!(store.list().is_empty());
}

// --- recent workspaces (RFC-003) ---

#[test]
fn recent_workspaces_persist_to_disk_and_reload() {
    let dir = temp_workspace();
    let file = dir.path().join("recents.json");
    let mut recents = RecentWorkspaces::default();
    recents.record(dir.path().to_path_buf(), "test".into(), 1000);
    recents.save(&file).unwrap();
    let loaded = RecentWorkspaces::load(&file);
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].root_path, dir.path());
}

#[test]
fn recent_workspaces_returns_default_when_file_absent() {
    let dir = temp_workspace();
    let loaded = RecentWorkspaces::load(&dir.path().join("nonexistent.json"));
    assert!(loaded.entries.is_empty());
}

// --- settings persistence (RFC-022) ---

#[test]
fn user_settings_persist_and_reload() {
    let dir = temp_workspace();
    let path = dir.path().join("settings.json");
    let s = UserSettings {
        autosave_debounce_ms: 2500,
        show_hidden_files: true,
        ..Default::default()
    };
    save_user_settings(&path, &s).unwrap();
    let loaded = load_user_settings(&path).unwrap();
    assert_eq!(loaded.autosave_debounce_ms, 2500);
    assert!(loaded.show_hidden_files);
}

#[test]
fn load_user_settings_returns_default_when_absent() {
    let dir = temp_workspace();
    let s = load_user_settings(&dir.path().join("no.json")).unwrap();
    assert_eq!(s, UserSettings::default());
}
