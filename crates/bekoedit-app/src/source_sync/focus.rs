use std::borrow::Cow;
use std::path::Path;
use std::time::Duration;

use bekoedit_core::AppState;
use bekoedit_ui_contract::{
    EditorMode,
    source_editor::{EditorIdentity, SourceEditorId},
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::toast::Toast;

use super::{
    SourceCommand, SourceSyncState, SubmitOutcome, controller::FocusClaim,
    controller::FocusResolution, submit_source_command, submit_source_command_preserving_focus,
};

const ARM_TIMEOUT: Duration = Duration::from_millis(250);
const FOCUS_GUARD_BOOTSTRAP: &str = include_str!("../../assets/focus-guard-bundle.js");
const FOCUS_GUARD_PROTOCOL_VERSION: u32 = 2;

/// Shell surfaces call this before moving DOM focus (RFC-042 §6.2 rule 1).
/// It routes through `acquire_shell_focus`, so every caller claims shell
/// focus authority — not just cancels a pending source-focus interaction.
/// Only call this for an actual shell surface with a close path that will
/// call `release_shell_focus` (menus, disclosure panels, screen
/// replacements). For a one-shot native OS dialog that isn't a shell
/// surface at all, use `cancel_pending_source_focus` instead — see the
/// review correction that split these (RFC-042 slice 1 re-review, C1/C2).
pub fn cancel_source_focus(mut sync: Signal<SourceSyncState>) {
    if let Some(token) = sync.write().acquire_shell_focus() {
        cancel_focus_guards_through(token);
    }
}

/// Cancels a pending source-focus interaction without claiming shell focus
/// authority. For call sites that briefly steal OS-level focus (a native
/// file dialog) but have no in-app close path to release authority from —
/// there is no shell surface here for RFC-042 §6 to arbitrate.
pub fn cancel_pending_source_focus(mut sync: Signal<SourceSyncState>) {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        cancel_focus_guards_through(token);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInteractionOrigin {
    kind: &'static str,
    invocation: &'static str,
    launch_id: Option<Cow<'static, str>>,
    current_mode: Option<EditorMode>,
    removal_policy: &'static str,
}

impl SourceInteractionOrigin {
    pub const fn persistent_control(launch_id: &'static str) -> Self {
        Self {
            kind: "persistentControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMustRemain",
        }
    }

    pub const fn removable_menu_control(launch_id: &'static str) -> Self {
        Self {
            kind: "removableMenuControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    pub const fn start_control(launch_id: &'static str) -> Self {
        Self {
            kind: "startControl",
            invocation: "pointer",
            launch_id: Some(Cow::Borrowed(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    /// A workspace-tree row opening a document (task 014 §3.1). The id is
    /// namespaced so it can never equal a fixed launch id, and the relative
    /// path keeps it unique among visible rows.
    pub fn tree_row(relative_path: &Path) -> Self {
        Self::document_link(format!("tree:{}", relative_path.display()))
    }

    /// A backlink opening its source document (task 014 §3.1). One file can
    /// link twice on one line, so the list position is part of the id.
    pub fn backlink(position: usize, source_path: &Path, line_number: usize) -> Self {
        Self::document_link(format!(
            "backlink:{position}:{}:{line_number}",
            source_path.display()
        ))
    }

    /// A search result opening its document (task 016). Like backlinks, one
    /// file can match twice on a line, so the list position is in the id.
    pub fn search_result(position: usize, relative_path: &Path, line_number: usize) -> Self {
        Self::document_link(format!(
            "search:{position}:{}:{line_number}",
            relative_path.display()
        ))
    }

    fn document_link(launch_id: String) -> Self {
        Self {
            kind: "documentLink",
            invocation: "pointer",
            launch_id: Some(Cow::Owned(launch_id)),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    pub fn launch_id(&self) -> Option<&str> {
        self.launch_id.as_deref()
    }

    fn shortcut(current_mode: EditorMode) -> Self {
        Self {
            kind: match current_mode {
                EditorMode::Text | EditorMode::Split => "replacedSourceSurface",
                EditorMode::Preview | EditorMode::Form => "replacedGeneralSurface",
            },
            invocation: "shortcut",
            launch_id: None,
            current_mode: Some(current_mode),
            removal_policy: "launchMayBeRemoved",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArmRequest<'a> {
    token: u64,
    fingerprint: &'a str,
    origin_kind: &'a str,
    invocation: &'a str,
    launch_id: Option<&'a str>,
    current_mode: Option<&'a str>,
    removal_policy: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuardArmed {
    token: u64,
    armed: bool,
    reason: Option<String>,
}

pub fn submit_source_interaction(
    sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    finalize_launch_ui: impl FnOnce() + 'static,
) {
    submit_interaction(
        sync,
        state,
        mode,
        toasts,
        command,
        origin,
        finalize_launch_ui,
    );
}

pub fn submit_source_shortcut_interaction(
    sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
) {
    let current_mode = *mode.read();
    submit_interaction(
        sync,
        state,
        mode,
        toasts,
        command,
        SourceInteractionOrigin::shortcut(current_mode),
        || {},
    );
}

fn submit_interaction(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    mode: Signal<EditorMode>,
    toasts: Signal<Vec<Toast>>,
    command: SourceCommand,
    origin: SourceInteractionOrigin,
    finalize_launch_ui: impl FnOnce() + 'static,
) {
    let current_mode = *mode.read();
    let target = focus_target(&command, current_mode);
    if target.is_none() || same_source_mode(&command, current_mode) {
        finalize_launch_ui();
        submit_source_command(sync, state, mode, toasts, command);
        return;
    }
    let target = target.expect("checked focus target");
    let fingerprint = format!(
        "{}:{}:{}:{}",
        origin.kind,
        origin.launch_id().unwrap_or("surface"),
        mode_name(current_mode),
        editor_name(target),
    );
    let Some((token, superseded)) = sync
        .write()
        .allocate_focus_interaction(target, fingerprint.clone())
    else {
        cancel_source_focus(sync);
        finalize_launch_ui();
        submit_source_command(sync, state, mode, toasts, command);
        return;
    };
    if let Some(old_token) = superseded {
        cancel_focus_guards_through(old_token);
    }
    crate::bridge::trace("source.focus.interaction.allocate", token);

    spawn(async move {
        let response =
            tokio::time::timeout(ARM_TIMEOUT, arm_focus_guard(token, &fingerprint, &origin))
                .await
                .ok()
                .flatten();
        let armed = response
            .as_ref()
            .is_some_and(|ack| ack.token == token && ack.armed);
        if let Some(ack) = response.as_ref()
            && !armed
        {
            crate::bridge::trace(
                "source.focus.guard.rejected",
                ack.reason.as_deref().unwrap_or("invalidAcknowledgement"),
            );
        } else if response.is_none() {
            crate::bridge::trace("source.focus.guard.timeout", token);
        }
        let resolution = if armed {
            FocusResolution::Armed
        } else {
            FocusResolution::ProceedWithoutFocus
        };
        if sync.write().claim_focus_interaction(token, resolution) == FocusClaim::Stale {
            cancel_focus_guards_through(token);
            return;
        }
        if armed {
            crate::bridge::trace("source.focus.guard.armed", token);
        } else {
            cancel_focus_guards_through(token);
        }
        finalize_launch_ui();
        let outcome =
            submit_source_command_preserving_focus(sync, state, mode, toasts, command, Some(token));
        crate::bridge::trace("source.focus.command.queued", format!("{outcome:?}"));
        if matches!(
            outcome,
            SubmitOutcome::NoOp | SubmitOutcome::Busy | SubmitOutcome::Unavailable
        ) {
            sync.write().cancel_focus_token(token);
            cancel_focus_guards_through(token);
        }
    });
}

/// The editor a command claims focus for. `OpenDocument` carries no mode and
/// does not switch one, so its claim follows the mode the app is already in
/// (task 014 §2); `NewUntitled` forces Text and `SwitchMode` names its mode.
fn focus_target(command: &SourceCommand, current_mode: EditorMode) -> Option<SourceEditorId> {
    let mode = match command {
        SourceCommand::NewUntitled => EditorMode::Text,
        SourceCommand::SwitchMode(target) => *target,
        SourceCommand::OpenDocument(_) => current_mode,
        _ => return None,
    };
    match mode {
        EditorMode::Text => Some(SourceEditorId::Text),
        EditorMode::Split => Some(SourceEditorId::Split),
        EditorMode::Preview | EditorMode::Form => None,
    }
}

fn same_source_mode(command: &SourceCommand, current: EditorMode) -> bool {
    matches!(
        (command, current),
        (
            SourceCommand::SwitchMode(EditorMode::Text),
            EditorMode::Text
        ) | (
            SourceCommand::SwitchMode(EditorMode::Split),
            EditorMode::Split
        )
    )
}

async fn arm_focus_guard(
    token: u64,
    fingerprint: &str,
    origin: &SourceInteractionOrigin,
) -> Option<GuardArmed> {
    let request = ArmRequest {
        token,
        fingerprint,
        origin_kind: origin.kind,
        invocation: origin.invocation,
        launch_id: origin.launch_id(),
        current_mode: origin.current_mode.map(mode_name),
        removal_policy: origin.removal_policy,
    };
    let payload = serde_json::to_string(&request).ok()?;
    let mut eval = document::eval(&format!(
        r#"
        {FOCUS_GUARD_BOOTSTRAP}
        (async () => {{
            const request = {payload};
            const guards = window.__bkFocusGuards;
            if (!guards
                || guards.protocolVersion !== {FOCUS_GUARD_PROTOCOL_VERSION}
                || typeof guards.arm !== "function") {{
                dioxus.send(JSON.stringify({{
                    token: request.token,
                    armed: false,
                    reason: "incompatibleRegistry",
                }}));
                return null;
            }}
            dioxus.send(JSON.stringify(guards.arm(request)));
            return null;
        }})();
        "#,
    ));
    let payload = eval.recv::<String>().await.ok()?;
    decode_guard_acknowledgement(&payload)
}

fn decode_guard_acknowledgement(payload: &str) -> Option<GuardArmed> {
    serde_json::from_str(payload).ok()
}

pub(crate) fn cancel_focus_guards_through(token: u64) {
    document::eval(&format!(
        r#"
        {FOCUS_GUARD_BOOTSTRAP}
        if (window.__bkFocusGuards?.protocolVersion === {FOCUS_GUARD_PROTOCOL_VERSION}
            && typeof window.__bkFocusGuards.cancelThrough === "function") {{
            window.__bkFocusGuards.cancelThrough({token});
        }}
        "#,
    ));
}

pub(crate) fn consume_focus_guard(token: u64, identity: EditorIdentity, fingerprint: &str) {
    let identity = serde_json::to_string(&identity).expect("editor identity serializes");
    let fingerprint = serde_json::to_string(fingerprint).expect("focus fingerprint serializes");
    document::eval(&format!(
        r#"
        if (window.__bk && typeof window.__bk.consumeFocusGuard === "function") {{
            window.__bk.consumeFocusGuard({{ token: {token}, identity: {identity}, fingerprint: {fingerprint} }});
        }}
        "#,
    ));
}

fn mode_name(mode: EditorMode) -> &'static str {
    match mode {
        EditorMode::Text => "text",
        EditorMode::Preview => "preview",
        EditorMode::Form => "form",
        EditorMode::Split => "split",
    }
}

fn editor_name(editor: SourceEditorId) -> &'static str {
    match editor {
        SourceEditorId::Text => "text",
        SourceEditorId::Split => "split",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_MODES: [EditorMode; 4] = [
        EditorMode::Text,
        EditorMode::Split,
        EditorMode::Preview,
        EditorMode::Form,
    ];

    fn claim_for(mode: EditorMode) -> Option<SourceEditorId> {
        match mode {
            EditorMode::Text => Some(SourceEditorId::Text),
            EditorMode::Split => Some(SourceEditorId::Split),
            EditorMode::Preview | EditorMode::Form => None,
        }
    }

    #[test]
    fn new_untitled_always_targets_the_text_editor() {
        for current in ALL_MODES {
            assert_eq!(
                focus_target(&SourceCommand::NewUntitled, current),
                Some(SourceEditorId::Text),
                "from {current:?}"
            );
        }
    }

    #[test]
    fn open_document_claims_focus_for_the_current_mode() {
        // Task 014 §3's table, one row per mode.
        let open = SourceCommand::OpenDocument("notes/a.md".into());
        assert_eq!(
            focus_target(&open, EditorMode::Text),
            Some(SourceEditorId::Text)
        );
        assert_eq!(
            focus_target(&open, EditorMode::Split),
            Some(SourceEditorId::Split)
        );
        assert_eq!(focus_target(&open, EditorMode::Preview), None);
        assert_eq!(focus_target(&open, EditorMode::Form), None);
    }

    #[test]
    fn switch_mode_claims_its_target_regardless_of_the_current_mode() {
        for current in ALL_MODES {
            for target in ALL_MODES {
                assert_eq!(
                    focus_target(&SourceCommand::SwitchMode(target), current),
                    claim_for(target),
                    "{current:?} -> {target:?}"
                );
            }
        }
    }

    #[test]
    fn commands_without_an_editor_destination_claim_nothing() {
        for current in ALL_MODES {
            for command in [
                SourceCommand::OpenSettings,
                SourceCommand::SaveNow,
                SourceCommand::CloseWorkspace,
            ] {
                assert_eq!(focus_target(&command, current), None, "{command:?}");
            }
        }
    }

    #[test]
    fn document_link_launch_ids_are_namespaced_and_position_unique() {
        let tree = SourceInteractionOrigin::tree_row(Path::new("notes/a.md"));
        assert_eq!(tree.launch_id(), Some("tree:notes/a.md"));
        assert_eq!(tree.invocation, "pointer");
        assert_eq!(tree.removal_policy, "launchMayBeRemoved");

        // Two links from one file on one line differ only by position.
        let first = SourceInteractionOrigin::backlink(0, Path::new("b.md"), 3);
        let second = SourceInteractionOrigin::backlink(1, Path::new("b.md"), 3);
        assert_eq!(first.launch_id(), Some("backlink:0:b.md:3"));
        assert_ne!(first.launch_id(), second.launch_id());
        assert_eq!(first.invocation, "pointer");
        assert_eq!(first.removal_policy, "launchMayBeRemoved");

        let search = SourceInteractionOrigin::search_result(2, Path::new("sub/child.md"), 1);
        assert_eq!(search.launch_id(), Some("search:2:sub/child.md:1"));
        assert_eq!(search.removal_policy, "launchMayBeRemoved");

        // Fixed ids keep their borrowed, unprefixed form.
        let fixed = SourceInteractionOrigin::start_control("start-new");
        assert_eq!(fixed.launch_id(), Some("start-new"));
        for id in [tree.launch_id(), first.launch_id()] {
            assert_ne!(id, fixed.launch_id());
        }
    }

    #[test]
    fn tree_and_backlink_opens_launch_a_focus_interaction() {
        // Task 014 §5.4: both call sites reach `focus_target` through
        // `submit_source_interaction` with their namespaced launch hook, rather
        // than the plain submit that cancels any focus claim.
        let tree_row = include_str!("../components/explorer/tree_row.rs");
        let backlinks = include_str!("../components/backlinks_panel.rs");
        for (name, source, origin) in [
            ("tree_row", tree_row, "SourceInteractionOrigin::tree_row("),
            ("backlinks", backlinks, "SourceInteractionOrigin::backlink("),
        ] {
            assert!(source.contains("submit_source_interaction("), "{name}");
            assert!(!source.contains("submit_source_command("), "{name}");
            assert!(source.contains(origin), "{name}");
            assert!(source.contains("\"data-source-focus-launch\":"), "{name}");
        }
    }

    #[test]
    fn guard_acknowledgement_decodes_from_the_javascript_string_payload() {
        let acknowledgement =
            decode_guard_acknowledgement(r#"{"token":7,"armed":true,"reason":null}"#)
                .expect("valid acknowledgement");

        assert_eq!(acknowledgement.token, 7);
        assert!(acknowledgement.armed);
        assert_eq!(acknowledgement.reason, None);
    }

    #[test]
    fn eager_guard_bundle_owns_arm_and_cancel_before_editor_bootstrap() {
        assert!(FOCUS_GUARD_BOOTSTRAP.contains("__bkFocusGuards"));
        assert!(FOCUS_GUARD_BOOTSTRAP.contains("protocolVersion"));
        assert!(FOCUS_GUARD_BOOTSTRAP.contains("consumeDiagnostic"));
        assert!(!FOCUS_GUARD_BOOTSTRAP.contains("CodeMirror"));
        assert_eq!(FOCUS_GUARD_PROTOCOL_VERSION, 2);
    }
}
