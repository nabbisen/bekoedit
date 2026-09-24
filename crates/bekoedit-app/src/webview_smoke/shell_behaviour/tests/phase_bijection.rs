//! A small module of its own so `tests.rs` stays under the ELOC guideline
//! (task 025 §5, mirroring `webview_smoke/tests/transport_guard.rs`'s own
//! reason). Task 025 §2.2's bijection test for RFC-044's run: the same
//! shape as `trusted_click/tests.rs`'s, generalised to this run's 28
//! phases. `ConflictBannerFocusKept`'s terminal arm was task 023's exact
//! bug, latent here only because `TERMINAL_STAGE`'s value happened to
//! equal the correct phase name (see `phase.rs`'s own doc comment on
//! `TERMINAL_STAGE`, fixed by task 025 §2.1).

use std::collections::BTreeSet;

use crate::webview_smoke::shell_behaviour::SHELL_BEHAVIOUR_JS;
use crate::webview_smoke::shell_behaviour::phase::ShellBehaviourPhase;
use crate::webview_smoke::transport::{parse_js_declared_phase_list, phases_via_next};

/// Every phase variant, matched exhaustively with no wildcard: adding a
/// variant without adding it here is a compile error, naming it -- paired
/// with the derived walk below (which only proves reachability via
/// `next()`, not that every declared variant was swept into it).
pub(super) const fn phase_count() -> usize {
    match ShellBehaviourPhase::RecoveryEntry {
        ShellBehaviourPhase::RecoveryEntry
        | ShellBehaviourPhase::RecoveryExit
        | ShellBehaviourPhase::DownUp
        | ShellBehaviourPhase::ExpandEnter
        | ShellBehaviourPhase::CollapseAscend
        | ShellBehaviourPhase::HomeEnd
        | ShellBehaviourPhase::NonOpenable
        | ShellBehaviourPhase::EnterOpens
        | ShellBehaviourPhase::SearchResultOpens
        | ShellBehaviourPhase::NewFileFocuses
        | ShellBehaviourPhase::TreeEnterAfterNewFile
        | ShellBehaviourPhase::FormSearchRestores
        | ShellBehaviourPhase::AppMenuMouseOpen
        | ShellBehaviourPhase::AppMenuKeys
        | ShellBehaviourPhase::AppMenuEscape
        | ShellBehaviourPhase::AppMenuFocusLeave
        | ShellBehaviourPhase::ToolsMenuKeys
        | ShellBehaviourPhase::ToolsMenuEscape
        | ShellBehaviourPhase::ToolsMenuFocusLeave
        | ShellBehaviourPhase::TabsArrowsFocusOnly
        | ShellBehaviourPhase::TabsClickActivates
        | ShellBehaviourPhase::MenuClosesIntoEditor
        | ShellBehaviourPhase::AuthorityReleasedAfterEditorFocus
        | ShellBehaviourPhase::QueuedSwitchClaimsFocus
        | ShellBehaviourPhase::SettingsEntry
        | ShellBehaviourPhase::SettingsExitRestored
        | ShellBehaviourPhase::ConflictDirtied
        | ShellBehaviourPhase::ConflictBannerFocusKept => {}
    }
    28
}

fn walked() -> BTreeSet<&'static str> {
    phases_via_next(
        ShellBehaviourPhase::RecoveryEntry,
        ShellBehaviourPhase::next,
        phase_count(),
    )
    .into_iter()
    .map(ShellBehaviourPhase::as_str)
    .collect()
}

#[test]
fn every_variant_is_reached_by_walking_next_from_the_first_phase() {
    let walked = walked();
    assert_eq!(
        walked.len(),
        phase_count(),
        "next() walk visited {} phases but phase_count() says there are {} -- \
         a variant exists that next() never reaches",
        walked.len(),
        phase_count()
    );
}

/// Task 025 §2.1/§2.2: `ShellBehaviourPhase::as_str()` must match
/// `shell_behaviour_driver.js`'s own `phases` array exactly.
/// `ConflictBannerFocusKept`'s terminal arm returning `TERMINAL_STAGE`
/// instead of its own phase-name literal was task 023's exact bug, present
/// here too and latent only because the two strings happened to be equal
/// today -- this test catches a future rename of either one.
#[test]
fn every_as_str_is_a_phase_the_driver_knows() {
    let driver_phases = parse_js_declared_phase_list(SHELL_BEHAVIOUR_JS);
    let rust_phases = walked();
    assert_eq!(
        rust_phases, driver_phases,
        "ShellBehaviourPhase::as_str() must match shell_behaviour_driver.js's own \
         phases array exactly -- a mismatch is a request the driver rejects before \
         its own try/catch, silently, for the shared transport's full 5 s round-trip cap"
    );
}
