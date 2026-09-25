//! The trusted-click run's fixture: two linked files in an isolated profile
//! (split out of `trusted_click.rs` to keep it under the 300-ELOC guideline,
//! task 029).

use std::path::PathBuf;

use bekoedit_fs::RecentWorkspaces;
use bekoedit_ui_contract::EditorMode;

use crate::persistence::AppPersistence;
use crate::settings::AppSettings;
use crate::webview_smoke::SmokeProfile;

pub(in crate::webview_smoke) struct PreparedTrustedClick {
    pub(in crate::webview_smoke) root: PathBuf,
    pub(in crate::webview_smoke) persistence: AppPersistence,
}

/// Creates an isolated profile and seeds the two-file fixture: `parent.md`
/// links to `child.md`, so `find_backlinks` reports `parent.md` once
/// `child.md` is open and the backlinks panel is asked to scan. Neither
/// file is opened here -- `TreeRowFocus`'s click is the first open, same
/// as `shell_behaviour.rs`'s `EnterOpens` contract relies on.
pub(in crate::webview_smoke) fn prepare(
    requested_root: &std::path::Path,
) -> Result<PreparedTrustedClick, String> {
    let profile = SmokeProfile::create(requested_root)?;

    let workspace = profile
        .persistence
        .isolated_paths()
        .expect("trusted-click persistence is always Isolated")
        .root()
        .join("workspace");
    std::fs::create_dir(&workspace)
        .map_err(|error| format!("cannot create trusted-click workspace: {error}"))?;
    std::fs::write(workspace.join("child.md"), "# child\n")
        .map_err(|error| format!("cannot seed trusted-click child.md: {error}"))?;
    std::fs::write(
        workspace.join("parent.md"),
        "# parent\n\nSee [child](./child.md) for details.\n",
    )
    .map_err(|error| format!("cannot seed trusted-click parent.md: {error}"))?;

    let settings = AppSettings {
        reopen_last_workspace: true,
        // See shell_behaviour.rs's `prepare`: AppState's real default is
        // Form (settings.rs), but §B's two phases need a CodeMirror text
        // view to assert focus into. §C then switches into Form itself as
        // its own setup click, so this only has to be right for §B.
        default_mode: EditorMode::Text,
        ..Default::default()
    };
    profile
        .persistence
        .save_settings(&settings)
        .map_err(|error| format!("cannot seed trusted-click settings: {error}"))?;

    let mut recents = RecentWorkspaces::default();
    recents.record(workspace, "workspace".to_string(), 1);
    recents
        .save(&profile.persistence.recents_file())
        .map_err(|error| format!("cannot seed trusted-click recents: {error}"))?;

    Ok(PreparedTrustedClick {
        root: profile.root,
        persistence: profile.persistence,
    })
}
