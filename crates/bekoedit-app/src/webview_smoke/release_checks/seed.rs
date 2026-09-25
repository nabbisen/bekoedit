//! Each scenario's own isolated profile: a workspace (or two), a recents
//! entry, and the reopen setting -- seeded exactly the way
//! `shell_behaviour::prepare` seeds its own, so no native dialog is involved.

use std::path::{Path, PathBuf};

use bekoedit_fs::RecentWorkspaces;
use bekoedit_ui_contract::EditorMode;

use crate::settings::AppSettings;
use crate::webview_smoke::SmokeProfile;

use super::link_judge::{NOTE, NOTE_FILE, OTHER_FILE};
use super::{Expectation, ReleaseChecksTerminal, ReleaseScenario};

pub(super) const SAVE_FILE: &str = "note.md";

/// CI creates the opener stubs and their log before the launch, and tells the
/// app where the log is with this variable (`link_clicks_reach_only_the_browser`).
pub(super) const OPENER_LOG_ENV: &str = "BEKOEDIT_LINK_OPENER_LOG";

/// Typed into the seeded file by the save scenario. Absent from the original.
pub(super) const EDIT_MARKER: &str = "ZQ7";

/// Mixed line endings (CRLF around one bare LF, and an unterminated last
/// line), plus constructs the parser preserves: a table, a fenced block and an
/// HTML comment. The first line is CRLF, and it is the one the scenario edits.
/// A uniform-CRLF file (every break `\r\n`, and a terminated last line), for
/// the scenario that exercises the uniform-file rule.
pub(super) fn original_crlf_note() -> Vec<u8> {
    concat!(
        "# Title\r\n",
        "\r\n",
        "Second line\r\n",
        "\r\n",
        "| a | b |\r\n",
        "|---|---|\r\n",
        "| 1 | 2 |\r\n",
        "\r\n",
        "```rust\r\n",
        "fn main() {}\r\n",
        "```\r\n",
        "\r\n",
        "<!-- a comment -->\r\n",
        "last line\r\n",
    )
    .as_bytes()
    .to_vec()
}

pub(super) fn original_note() -> Vec<u8> {
    concat!(
        "# Title\r\n",
        "plain LF line\n",
        "second CRLF line\r\n",
        "\r\n",
        "| a | b |\r\n",
        "|---|---|\r\n",
        "| 1 | 2 |\r\n",
        "\r\n",
        "```rust\r\n",
        "fn main() {}\r\n",
        "```\r\n",
        "\r\n",
        "<!-- a comment -->\r\n",
        "last line, no terminator",
    )
    .as_bytes()
    .to_vec()
}

fn make_workspace(root: &Path, name: &str, files: &[(&str, &[u8])]) -> Result<PathBuf, String> {
    let workspace = root.join(name);
    std::fs::create_dir(&workspace)
        .map_err(|error| format!("cannot create release-checks workspace {name}: {error}"))?;
    for (file, bytes) in files {
        std::fs::write(workspace.join(file), bytes)
            .map_err(|error| format!("cannot seed release-checks {name}/{file}: {error}"))?;
    }
    Ok(workspace)
}

pub(in crate::webview_smoke) fn prepare(
    requested_root: &Path,
    scenario: ReleaseScenario,
) -> Result<(SmokeProfile, ReleaseChecksTerminal), String> {
    let profile = SmokeProfile::create(requested_root)?;
    let root = profile
        .persistence
        .isolated_paths()
        .expect("release-checks persistence is always Isolated")
        .root()
        .to_path_buf();

    let plain: [(&str, &[u8]); 2] = [("a.md", b"# a\n"), ("b.md", b"# b\n")];
    let mut recents = RecentWorkspaces::default();
    let mut older_workspace = None;
    let (workspace, display_name, original, file) = match scenario {
        ReleaseScenario::ReopenUsable | ReleaseScenario::ReopenDisabled => (
            make_workspace(&root, "usable-project", &plain)?,
            "Usable Project".to_string(),
            Vec::new(),
            None,
        ),
        ReleaseScenario::ReopenMissing => {
            // The older entry is still usable: falling back to it is exactly
            // what RFC-043 §9 forbids, and what "no other project opened"
            // means. `record` puts the newest first, so the missing one goes
            // in last.
            let older = make_workspace(&root, "older-project", &plain)?;
            recents.record(older.clone(), "Older Project".into(), 1);
            older_workspace = Some(older);
            (
                make_workspace(&root, "gone-project", &plain)?,
                "Gone Project".to_string(),
                Vec::new(),
                None,
            )
        }
        ReleaseScenario::LinkClicksReachOnlyTheBrowser => (
            // `other.md` really exists, so a relative link that leaked would
            // have a target for the opener to be given.
            make_workspace(
                &root,
                "link-project",
                &[(NOTE_FILE, NOTE.as_bytes()), (OTHER_FILE, b"# other\n")],
            )?,
            "Link Project".to_string(),
            Vec::new(),
            None,
        ),
        ReleaseScenario::SavePreservesBytes
        | ReleaseScenario::SavePreservesCrlfBytes
        | ReleaseScenario::ModeSwitchPreservesBytes => {
            let original = if scenario == ReleaseScenario::SavePreservesCrlfBytes {
                original_crlf_note()
            } else {
                original_note()
            };
            let workspace = make_workspace(&root, "save-project", &[(SAVE_FILE, &original)])?;
            let file = workspace.join(SAVE_FILE);
            (workspace, "Save Project".to_string(), original, Some(file))
        }
    };
    recents.record(workspace.clone(), display_name.clone(), 2);

    let settings = AppSettings {
        reopen_last_workspace: scenario != ReleaseScenario::ReopenDisabled,
        // Same reason as `shell_behaviour::prepare`: the save scenario needs a
        // CodeMirror text view to type into.
        default_mode: if scenario == ReleaseScenario::LinkClicksReachOnlyTheBrowser {
            // The links are clicked in the rendered note.
            EditorMode::Preview
        } else {
            EditorMode::Text
        },
        ..Default::default()
    };
    profile
        .persistence
        .save_settings(&settings)
        .map_err(|error| format!("cannot seed release-checks settings: {error}"))?;
    recents
        .save(&profile.persistence.recents_file())
        .map_err(|error| format!("cannot seed release-checks recents: {error}"))?;

    if scenario == ReleaseScenario::ReopenMissing {
        // Renamed away only after the recents entry was written, so the
        // entry names a folder that no longer exists.
        std::fs::rename(&workspace, root.join("gone-project-renamed"))
            .map_err(|error| format!("cannot rename the seeded workspace away: {error}"))?;
    }

    let opener_log = if scenario == ReleaseScenario::LinkClicksReachOnlyTheBrowser {
        Some(std::env::var_os(OPENER_LOG_ENV).map(PathBuf::from).ok_or_else(|| {
            format!(
                "{}: {OPENER_LOG_ENV} is not set; run scripts/webview-link-opener-stubs.sh first \
                 and pass its log path and BROWSER through the environment",
                scenario.name()
            )
        })?)
    } else {
        None
    };
    let expectation = Expectation {
        workspace,
        display_name,
        older_workspace,
        file,
        original,
        opener_log,
    };
    Ok((profile, ReleaseChecksTerminal::new(scenario, expectation)))
}
