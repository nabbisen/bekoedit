//! Task 023's phase table: every phase, its order, its milestone, and the
//! terminal stage, kept together per the same convention as
//! `shell_behaviour/phase.rs` (RFC-044 slice 3 handoff §4.4).
//!
//! Three phases, one real XTEST click each (task 023 §2/§3): a plain,
//! always-present control first (`ProofOfTrust`, the claim §5.1 needs
//! before anything else can be trusted), then §B's two manual-walkthrough
//! checks this run covers -- the workspace-tree row and backlink
//! (`TreeRowFocus`, `BacklinkFocus`, terminal).
//!
//! §C (Form mode, click the Text tab) is deliberately not here. Task 023's
//! review (2026-09-23) found its phase never completes on real CI -- a
//! `document::eval` issued after a source-editor unmount-then-remount
//! (Form, then Text) hangs for the shared transport's full round-trip cap
//! and never answers, regardless of how long the harness waits first. Its
//! code lived on this run's branch as `ModeTabFocus`, at tip `6a0c861`;
//! task 024 (`.git-exclude/tasks/dev-team/024-eval-after-remount.md`)
//! investigates the hang, from a fresh branch, before that phase returns
//! here or anywhere else.

use crate::webview_smoke::transport::PhaseKind;

pub(super) const EXPECTED_MILESTONES: [&str; 3] = [
    "trusted_click_focused_default_target",
    "tree_row_trusted_click_focused_editor",
    "backlink_trusted_click_focused_editor",
];

/// The phase whose success is the whole run's terminal result -- §B's
/// second item.
pub(super) const TERMINAL_STAGE: &str = "backlink_trusted_click_focused_editor";

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
    /// and the editor takes focus. Terminal.
    BacklinkFocus,
}

impl TrustedClickPhase {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "proof_of_trust",
            Self::TreeRowFocus => "tree_row_focus",
            Self::BacklinkFocus => TERMINAL_STAGE,
        }
    }

    pub(super) const fn next(self) -> Option<Self> {
        match self {
            Self::ProofOfTrust => Some(Self::TreeRowFocus),
            Self::TreeRowFocus => Some(Self::BacklinkFocus),
            Self::BacklinkFocus => None,
        }
    }

    /// The `milestone` a `Progress` report from this phase must carry --
    /// one-to-one with `EXPECTED_MILESTONES`. The terminal phase reports
    /// its milestone via `DriverResult.milestones`.
    pub(super) const fn expected_milestone(self) -> &'static str {
        match self {
            Self::ProofOfTrust => "trusted_click_focused_default_target",
            Self::TreeRowFocus => "tree_row_trusted_click_focused_editor",
            Self::BacklinkFocus => TERMINAL_STAGE,
        }
    }
}

impl PhaseKind for TrustedClickPhase {
    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }
}
