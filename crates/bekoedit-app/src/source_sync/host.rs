use bekoedit_core::AppState;
use bekoedit_ui_contract::{
    BRIDGE_SCHEMA_VERSION, EditorMode,
    source_editor::{BridgeFailureReason, SourceEditorEvent, SourceEditorId, SourceEditorRequest},
};
use dioxus::prelude::*;
use serde::Deserialize;

use crate::{
    bridge,
    components::toast::{Toast, ToastKind, push_toast},
    state::now_ms,
};

use super::{
    SourceSyncError, commands,
    controller::{ControllerAction, EventOutcome, SourceSyncState, TickOutcome, fingerprint},
    lifecycle::LifecycleEffect,
};

const EDITOR_BUNDLE: Asset = asset!("/assets/editor-bundle.js");
const SOURCE_RELAY: &str = "__bk_source_editor_relay";
const TEXT_CONTAINER: &str = "cm-root";
const SPLIT_CONTAINER: &str = "cm-split";

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AuxiliaryEvent {
    Scroll { fraction: f64 },
    RelayGenerationReady { generation: u64 },
}

#[component]
pub fn SourceEditorControllerHost() -> Element {
    let mut sync = use_context::<Signal<SourceSyncState>>();
    let mut state = use_context::<Signal<AppState>>();
    let mut mode = use_context::<Signal<EditorMode>>();
    let mut settings_open = use_context::<Signal<bool>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        sync.write().start_bundle_probe(now_ms());
        let mut consecutive_failures = 0_u32;
        let mut generation = 0_u64;
        loop {
            generation = generation.saturating_add(1);
            sync.write().relay_generation_started(generation);
            let relay_js = bridge::relay_js(SOURCE_RELAY, generation);
            let mut relay = document::eval(&relay_js);
            while let Ok(raw) = relay.recv::<serde_json::Value>().await {
                consecutive_failures = 0;
                if let Ok(event) = decode::<SourceEditorEvent>(raw.clone()) {
                    let handled = {
                        let mut app = state.write();
                        sync.write().handle_event(event, &mut app, now_ms())
                    };
                    match handled {
                        Ok(EventOutcome::Applied) => {}
                        Ok(EventOutcome::Stale) => bridge::trace("source.controller.stale", ""),
                        Err(error) => announce_error(toasts, error),
                    }
                } else if let Ok(auxiliary) = decode::<AuxiliaryEvent>(raw) {
                    match auxiliary {
                        AuxiliaryEvent::Scroll { fraction } => mirror_split_scroll(fraction),
                        AuxiliaryEvent::RelayGenerationReady {
                            generation: acknowledged,
                        } if acknowledged == generation => {
                            let now = now_ms();
                            if sync.write().relay_generation_ready(generation, now) {
                                sync.write().start_bundle_probe(now);
                            }
                        }
                        AuxiliaryEvent::RelayGenerationReady { .. } => {}
                    }
                }
            }
            document::eval(&bridge::clear_relay_js(SOURCE_RELAY, generation));
            if sync.write().relay_disconnected(generation) {
                announce_error(toasts, SourceSyncError::EditorUnavailable);
            }
            consecutive_failures = consecutive_failures.saturating_add(1);
            let delay_ms = bridge::relay_restart_delay_ms(consecutive_failures);
            bridge::trace("source.relay.restart", consecutive_failures);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    });

    use_effect(move || {
        if !sync.read().has_dispatchable_actions() {
            return;
        }
        let actions = sync.write().drain_dispatchable_actions();
        for action in actions {
            match action {
                ControllerAction::Lifecycle(effect) => {
                    let generation = sync
                        .read()
                        .relay_generation()
                        .expect("lifecycle actions require a ready relay generation");
                    dispatch_lifecycle_effect(sync, state, toasts, effect, generation);
                }
                ControllerAction::Execute { command, protected } => {
                    let result = {
                        let mut app = state.write();
                        let mut editor_mode = mode.write();
                        let mut settings = settings_open.write();
                        commands::execute(
                            &mut app,
                            &mut editor_mode,
                            &mut settings,
                            &command,
                            now_ms(),
                        )
                    };
                    let success = result.is_ok();
                    match result {
                        Ok(Some(notice)) => {
                            let mut target = toasts;
                            push_toast(&mut target, notice.kind, notice.message);
                        }
                        Ok(None) => {}
                        Err(error) => announce_error(toasts, error.into()),
                    }
                    if protected {
                        let after = fingerprint(&state.read());
                        if let Err(error) = sync.write().command_completed(success, after, now_ms())
                        {
                            announce_error(toasts, error);
                        }
                    }
                }
            }
        }
    });

    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            match sync.write().tick(now_ms()) {
                Ok(TickOutcome::TimedOut) => announce_error(toasts, SourceSyncError::Timeout),
                Ok(TickOutcome::Idle | TickOutcome::TakeoverStarted) => {}
                Err(error) => announce_error(toasts, error),
            }
        }
    });

    use_drop(move || {
        let effect = { sync.write().shutdown(now_ms()) };
        let generation = sync.read().relay_generation();
        if let (Some(effect), Some(generation)) = (effect, generation) {
            dispatch_lifecycle_effect(sync, state, toasts, effect, generation);
        }
    });

    rsx! { document::Script { src: EDITOR_BUNDLE } }
}

fn dispatch_lifecycle_effect(
    mut sync: Signal<SourceSyncState>,
    state: Signal<AppState>,
    toasts: Signal<Vec<Toast>>,
    effect: LifecycleEffect,
    relay_generation: u64,
) {
    let Some(request) = request_for_effect(&effect, &state.read()) else {
        sync.write().force_unmount(now_ms());
        announce_error(toasts, SourceSyncError::EditorUnavailable);
        return;
    };
    let fallback = match effect {
        LifecycleEffect::ProbeBundle(operation_id) => Some(SourceEditorEvent::BundleFailed {
            protocol_version: BRIDGE_SCHEMA_VERSION,
            operation_id,
            reason: BridgeFailureReason::BridgeError,
        }),
        LifecycleEffect::InstallRelay(identity, operation_id) => {
            Some(SourceEditorEvent::RelayFailed {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id,
                identity,
                reason: BridgeFailureReason::BridgeError,
            })
        }
        _ => None,
    };
    dispatch_request(&request, fallback.as_ref(), relay_generation);
}

fn request_for_effect(effect: &LifecycleEffect, app: &AppState) -> Option<SourceEditorRequest> {
    match effect {
        LifecycleEffect::ProbeBundle(operation_id) => {
            Some(SourceEditorRequest::current_probe(*operation_id))
        }
        LifecycleEffect::InstallRelay(identity, operation_id) => {
            Some(SourceEditorRequest::InstallRelay {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                identity: *identity,
            })
        }
        LifecycleEffect::Init(identity, operation_id, takeover) => {
            let session = app
                .session
                .as_ref()
                .filter(|session| session.document_id == identity.document_id)?;
            Some(SourceEditorRequest::InitEditor {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                identity: *identity,
                container_id: container_id(identity.editor_id).to_string(),
                revision: session.revision,
                text: session.canonical_text.clone(),
                takeover: takeover.clone(),
            })
        }
        LifecycleEffect::RequestSnapshot(identity, operation_id) => {
            Some(SourceEditorRequest::RequestSnapshot {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                identity: *identity,
            })
        }
        LifecycleEffect::Resume(identity, snapshot_operation_id, operation_id) => {
            let revision = app
                .session
                .as_ref()
                .filter(|session| session.document_id == identity.document_id)?
                .revision;
            Some(SourceEditorRequest::ResumeEditing {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                identity: *identity,
                snapshot_operation_id: *snapshot_operation_id,
                revision,
            })
        }
        LifecycleEffect::ApplyDocument(old_identity, new_epoch, operation_id) => {
            let session = app
                .session
                .as_ref()
                .filter(|session| session.document_id == old_identity.document_id)?;
            Some(SourceEditorRequest::ApplyDocument {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                old_identity: *old_identity,
                new_epoch: *new_epoch,
                revision: session.revision,
                text: session.canonical_text.clone(),
            })
        }
        LifecycleEffect::Destroy(identity, operation_id) => {
            Some(SourceEditorRequest::DestroyEditor {
                protocol_version: BRIDGE_SCHEMA_VERSION,
                operation_id: *operation_id,
                identity: *identity,
            })
        }
        LifecycleEffect::ExecuteCommand(_) => None,
    }
}

fn dispatch_request(
    request: &SourceEditorRequest,
    fallback: Option<&SourceEditorEvent>,
    relay_generation: u64,
) {
    let payload = serde_json::to_string(request).expect("typed bridge request serializes");
    let fallback =
        fallback.map(|event| serde_json::to_string(event).expect("typed bridge event serializes"));
    document::eval(&dispatch_request_js(
        &payload,
        fallback.as_deref(),
        relay_generation,
    ));
}

fn dispatch_request_js(payload: &str, fallback: Option<&str>, relay_generation: u64) -> String {
    if let Some(fallback) = fallback {
        format!(
            r#"
            (async () => {{
                const request = {payload};
                for (let attempt = 0; attempt < 80; attempt += 1) {{
                    if (window.__bk && window.__bk.protocolVersion === {version}
                        && typeof window.__bk.dispatchForRelayGeneration === "function"
                        && window.__bk.dispatchForRelayGeneration(request, {generation})) {{
                        return;
                    }}
                    await new Promise(resolve => setTimeout(resolve, 50));
                }}
                const relay = window.{relay};
                if (typeof relay === "function"
                    && relay.__bkGeneration === {generation}) {{
                    relay(JSON.stringify({fallback}));
                }}
            }})();
            "#,
            version = BRIDGE_SCHEMA_VERSION,
            relay = SOURCE_RELAY,
            generation = relay_generation,
        )
    } else {
        format!(
            r#"
            (async () => {{
                const request = {payload};
                for (let attempt = 0; attempt < 20; attempt += 1) {{
                    if (window.__bk && window.__bk.protocolVersion === {version}
                        && typeof window.__bk.dispatchForRelayGeneration === "function"
                        && window.__bk.dispatchForRelayGeneration(request, {generation})) {{
                        return;
                    }}
                    await new Promise(resolve => setTimeout(resolve, 25));
                }}
            }})();
            "#,
            version = BRIDGE_SCHEMA_VERSION,
            generation = relay_generation,
        )
    }
}

fn mirror_split_scroll(fraction: f64) {
    let js = format!(
        r#"
        const preview = document.getElementById("preview-split");
        if (preview) {{
            const max = preview.scrollHeight - preview.clientHeight;
            preview.scrollTop = max * {fraction};
        }}
        "#
    );
    document::eval(&js);
}

fn decode<T: serde::de::DeserializeOwned>(raw: serde_json::Value) -> serde_json::Result<T> {
    if let Some(json) = raw.as_str() {
        serde_json::from_str(json)
    } else {
        serde_json::from_value(raw)
    }
}

fn container_id(editor_id: SourceEditorId) -> &'static str {
    match editor_id {
        SourceEditorId::Text => TEXT_CONTAINER,
        SourceEditorId::Split => SPLIT_CONTAINER,
    }
}

fn announce_error(mut toasts: Signal<Vec<Toast>>, error: SourceSyncError) {
    if matches!(
        error,
        SourceSyncError::Transition(TransitionError::Stale | TransitionError::InvalidState)
    ) {
        bridge::trace("source.controller.stale_error", error);
        return;
    }
    push_toast(&mut toasts, ToastKind::Error, error.to_string());
}

use super::lifecycle::TransitionError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_dispatch_requires_the_acknowledged_exact_generation() {
        for script in [
            dispatch_request_js("{}", None, 41),
            dispatch_request_js("{}", Some("{}"), 41),
        ] {
            assert!(script.contains("window.__bk.dispatchForRelayGeneration(request, 41)"));
            assert!(!script.contains("__bkGeneration === 40"));
        }
    }
}
