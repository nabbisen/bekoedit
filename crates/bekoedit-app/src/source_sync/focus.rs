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

pub fn cancel_source_focus(mut sync: Signal<SourceSyncState>) {
    if let Some(token) = sync.write().cancel_focus_interactions() {
        cancel_focus_guards_through(token);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInteractionOrigin {
    kind: &'static str,
    invocation: &'static str,
    launch_id: Option<&'static str>,
    current_mode: Option<EditorMode>,
    removal_policy: &'static str,
}

impl SourceInteractionOrigin {
    pub const fn persistent_control(launch_id: &'static str) -> Self {
        Self {
            kind: "persistentControl",
            invocation: "pointer",
            launch_id: Some(launch_id),
            current_mode: None,
            removal_policy: "launchMustRemain",
        }
    }

    pub const fn removable_menu_control(launch_id: &'static str) -> Self {
        Self {
            kind: "removableMenuControl",
            invocation: "pointer",
            launch_id: Some(launch_id),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
    }

    pub const fn start_control(launch_id: &'static str) -> Self {
        Self {
            kind: "startControl",
            invocation: "pointer",
            launch_id: Some(launch_id),
            current_mode: None,
            removal_policy: "launchMayBeRemoved",
        }
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
    let target = focus_target(&command);
    if target.is_none() || same_source_mode(&command, current_mode) {
        finalize_launch_ui();
        submit_source_command(sync, state, mode, toasts, command);
        return;
    }
    let target = target.expect("checked focus target");
    let fingerprint = format!(
        "{}:{}:{}:{}",
        origin.kind,
        origin.launch_id.unwrap_or("surface"),
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

fn focus_target(command: &SourceCommand) -> Option<SourceEditorId> {
    let mode = match command {
        SourceCommand::NewUntitled => EditorMode::Text,
        SourceCommand::SwitchMode(target) => *target,
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
        launch_id: origin.launch_id,
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

    #[test]
    fn new_untitled_always_targets_the_text_editor() {
        assert_eq!(
            focus_target(&SourceCommand::NewUntitled),
            Some(SourceEditorId::Text)
        );
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
