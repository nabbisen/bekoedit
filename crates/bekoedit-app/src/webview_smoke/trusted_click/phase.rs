//! Task 023's phase table: every phase, its order, its milestone, and the
//! terminal stage, kept together per the same convention as
//! `shell_behaviour/phase.rs` (RFC-044 slice 3 handoff §4.4).
//!
//! Four phases, one real XTEST click each (task 023 §2/§3): a plain,
//! always-present control first (`ProofOfTrust`, the claim §5.1 needs
//! before anything else can be trusted), then the two manual-walkthrough
//! checks this task exists to automate -- §B's workspace-tree row and
//! backlink (`TreeRowFocus`, `BacklinkFocus`), then §C's mode tab
//! (`ModeTabFocus`, terminal -- "the check this file exists for").
//!
//! `as_str()` and `TERMINAL_STAGE` are two different concepts that must
//! stay two different strings: `as_str()` is the phase *name*, sent as
//! `PhaseRequest.phase` and matched against `trusted_click_driver.js`'s
//! own `phases` array; `TERMINAL_STAGE` is the *result stage* name the
//! terminal phase's final `DriverResult.stage` is checked against. Every
//! run of this branch through 2026-09-23 (sixteen real CI runs) had the
//! terminal phase's `as_str()` return `TERMINAL_STAGE` instead of its own
//! name -- a request the driver rejected before its own `try`/`catch`,
//! silently, with nothing ever sent back, so Rust waited out the shared
//! transport's 5 s cap every time regardless of what phase was terminal
//! or what its own click did. `every_as_str_is_a_phase_the_driver_knows`
//! (tests.rs) is a bijection test against the driver's own source,
//! specifically so this cannot come back quietly.

use crate::webview_smoke::transport::PhaseKind;

pub(super) const EXPECTED_MILESTONES: [&str; 4] = [
    "trusted_click_focused_default_target",
    "tree_row_trusted_click_focused_editor",
    "backlink_trusted_click_focused_editor",
    "mode_tab_trusted_click_focused_editor",
];

/// The result stage the terminal phase's `DriverResult.stage` must equal
/// -- not a phase name; see this module's own doc comment.
pub(super) const TERMINAL_STAGE: &str = "mode_tab_trusted_click_focused_editor";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::webview_smoke) enum TrustedClickPhase {
    /// §5.1: an XTEST click on a plain, always-present control
    /// (`#app-menu-trigger`) moves `document.activeElement` to it. No
    /// bekoedit focus-guard logic is in the loop -- this proves the
    /// trusted-vs-synthetic click theory in isolation, before any later
    /// phase depends on it.
    ProofOfTrust,
    /// §B item 1: a trusted click on a workspace-tree row opens its
    /// document and the editor takes focus.
    TreeRowFocus,
    /// §B item 2: a trusted click on a backlink opens its source document
    /// and the editor takes focus.
    BacklinkFocus,
    /// §C: with a document open in Form mode, a trusted click on the Text
    /// mode tab switches to it and the editor takes focus. Terminal --
    /// "the check this file exists for".
    ModeTabFocus,
}

impl TrustedClickPhase {
    /// This phase's own name, sent as `PhaseRequest.phase` and matched
    /// against `trusted_click_driver.js`'s `phases` array -- not
    /// `TERMINAL_STAGE` for the terminal variant; see this module's own
    /// doc comment for what went wrong when it was.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "proof_of_trust",
            Self::TreeRowFocus => "tree_row_focus",
            Self::BacklinkFocus => "backlink_focus",
            Self::ModeTabFocus => "mode_tab_focus",
        }
    }

    pub(super) const fn next(self) -> Option<Self> {
        match self {
            Self::ProofOfTrust => Some(Self::TreeRowFocus),
            Self::TreeRowFocus => Some(Self::BacklinkFocus),
            Self::BacklinkFocus => Some(Self::ModeTabFocus),
            Self::ModeTabFocus => None,
        }
    }

    /// The `milestone` a `Progress` report from this phase must carry --
    /// one-to-one with `EXPECTED_MILESTONES`. The terminal phase reports
    /// its milestone via `DriverResult.milestones`, not this -- using
    /// `TERMINAL_STAGE` here is harmless (it's the correct final
    /// milestone string, and `Terminal` messages never carry a
    /// `milestone` field for `validate()` to check it against).
    pub(super) const fn expected_milestone(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "trusted_click_focused_default_target",
            Self::TreeRowFocus => "tree_row_trusted_click_focused_editor",
            Self::BacklinkFocus => "backlink_trusted_click_focused_editor",
            Self::ModeTabFocus => TERMINAL_STAGE,
        }
    }
}

impl PhaseKind for TrustedClickPhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}
