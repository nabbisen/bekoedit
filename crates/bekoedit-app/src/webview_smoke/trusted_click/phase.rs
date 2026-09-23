//! Task 023's phase table: every phase, its order, its milestone, and the
//! terminal stage, kept together per the same convention as
//! `shell_behaviour/phase.rs` (RFC-044 slice 3 handoff §4.4).
//!
//! Four phases: a plain, always-present control first (`ProofOfTrust`,
//! the claim §5.1 needs before anything else can be trusted), §B's two
//! manual-walkthrough checks this run covers -- the workspace-tree row
//! and backlink (`TreeRowFocus`, `BacklinkFocus`) -- then a diagnostic
//! `NoOpTerminal`, terminal, with no click of its own.
//!
//! `NoOpTerminal` is the discriminating experiment the 2026-09-23 finding
//! review's §4.3 asked for, not a design choice: every real run so far
//! has hung at whichever phase is *last*, regardless of which phase that
//! is or what its own click does -- `mode_tab_focus` when it was last,
//! then `backlink_focus` once `mode_tab_focus` was removed and it became
//! last instead, unchanged even after fixing the two harness defects the
//! review found (§3.1, §3.2). If `NoOpTerminal` -- no click, nothing to
//! wait for, first query should report success at once -- also hangs,
//! being terminal is implicated regardless of clicks. If it passes and
//! `BacklinkFocus` (now non-terminal) also passes reliably, the trigger
//! is specifically the last *click-performing* phase. Do not land §B
//! ending on this phase; per the review, remove it once the cause is
//! understood.
//!
//! §C (Form mode, click the Text tab) is deliberately not here. Task 023's
//! first review (2026-09-23) found its phase never completed on real CI;
//! task 024, built on a since-withdrawn hypothesis about why, is also
//! withdrawn (2026-09-23 finding review §1). The investigation continues
//! in task 023 itself. Its code last lived on this branch as
//! `ModeTabFocus`, at tip `6a0c861`.

use crate::webview_smoke::transport::PhaseKind;

pub(super) const EXPECTED_MILESTONES: [&str; 4] = [
    "trusted_click_focused_default_target",
    "tree_row_trusted_click_focused_editor",
    "backlink_trusted_click_focused_editor",
    "no_op_terminal_reported",
];

/// The phase whose success is the whole run's terminal result --
/// diagnostic only; see this module's own doc comment.
pub(super) const TERMINAL_STAGE: &str = "no_op_terminal_reported";

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
    /// Diagnostic (see this module's own doc comment): no click, reports
    /// success on its first query. Terminal.
    NoOpTerminal,
}

impl TrustedClickPhase {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "proof_of_trust",
            Self::TreeRowFocus => "tree_row_focus",
            Self::BacklinkFocus => "backlink_focus",
            Self::NoOpTerminal => TERMINAL_STAGE,
        }
    }

    pub(super) const fn next(self) -> Option<Self> {
        match self {
            Self::ProofOfTrust => Some(Self::TreeRowFocus),
            Self::TreeRowFocus => Some(Self::BacklinkFocus),
            Self::BacklinkFocus => Some(Self::NoOpTerminal),
            Self::NoOpTerminal => None,
        }
    }

    /// The `milestone` a `Progress` report from this phase must carry --
    /// one-to-one with `EXPECTED_MILESTONES`. The terminal phase reports
    /// its milestone via `DriverResult.milestones`.
    pub(super) const fn expected_milestone(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "trusted_click_focused_default_target",
            Self::TreeRowFocus => "tree_row_trusted_click_focused_editor",
            Self::BacklinkFocus => "backlink_trusted_click_focused_editor",
            Self::NoOpTerminal => TERMINAL_STAGE,
        }
    }
}

impl PhaseKind for TrustedClickPhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}
