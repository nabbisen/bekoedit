//! Root application component.

use crate::bridge;
use std::path::PathBuf;

use dioxus::prelude::*;
use serde::Deserialize;

use bekoedit_core::AppState;
use bekoedit_fs::{FsWatcher, WatchEvent};
use bekoedit_ui_contract::EditorMode;

use crate::components::{
    app_bar::{AppBar, release_menu_focus},
    backlinks_panel::BacklinksPanel,
    conflict_banner::ConflictBanner,
    editor_header::EditorHeader,
    explorer::Explorer,
    form_mode::FormMode,
    history_panel::HistoryPanel,
    outline_panel::OutlinePanel,
    preview_mode::PreviewMode,
    recovery_screen::RecoveryScreen,
    settings_screen::SettingsScreen,
    split_mode::SplitMode,
    start_screen::StartScreen,
    status_bar::StatusBar,
    text_mode::TextMode,
    toast::{Toast, ToastKind, ToastLayer, push_toast},
};
use crate::i18n::{Lang, tr};
use crate::source_sync::host::SourceEditorControllerHost;
use crate::source_sync::{
    SourceCommand, SourceSyncState, submit_source_command, submit_source_shortcut_interaction,
};
use crate::state::{
    BacklinksOpen, ExplorerCollapsed, HistoryOpen, NewFileOpen, OpenMenu, OpenMenuState,
    OutlineOpen, SearchOpen, SettingsOpen, create_app_state, now_ms,
};
use crate::webview_smoke::{WebViewKeySpikeDriver, WebViewSmokeDriver, launch_config};

pub(crate) const STYLE_SOURCE: &str = include_str!("../assets/style.css");
pub(crate) const SHORTCUTS_SOURCE: &str = include_str!("../assets/shortcuts.js");
const TICK_MS: u64 = 500;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum AppMsg {
    Shortcut { key: String },
}

#[component]
pub fn App() -> Element {
    let launch = launch_config();
    let persistence = launch.persistence.clone();
    let settings = persistence.load_settings();
    let webview_smoke = launch.webview_smoke;
    let webview_key_spike = launch.key_spike.is_some();

    use_context_provider(|| persistence.clone());
    // RFC-043: the workspace may already be open in `initial_state` when
    // this closure runs (once, on mount) -- computed and reopen_failure
    // captured together so `create_app_state`'s attempt-and-decide (§3.5:
    // no parallel usability check) runs exactly once, not once per
    // render.
    let (state, reopen_failure) = use_hook(|| {
        let (initial_state, notice) = create_app_state(&persistence, &settings);
        (Signal::new(initial_state), notice)
    });
    use_context_provider(|| state);
    use_context_provider(|| Signal::new(settings.lang));
    use_context_provider(|| Signal::new(settings.default_mode));
    use_context_provider(|| ExplorerCollapsed(Signal::new(false_val())));
    use_context_provider(|| SettingsOpen(Signal::new(false_val())));
    use_context_provider(|| OutlineOpen(Signal::new(false_val())));
    use_context_provider(|| SearchOpen(Signal::new(false_val())));
    use_context_provider(|| BacklinksOpen(Signal::new(false_val())));
    use_context_provider(|| HistoryOpen(Signal::new(false_val())));
    use_context_provider(|| NewFileOpen(Signal::new(false_val())));
    let mut open_menu = use_context_provider(|| OpenMenuState(Signal::new(OpenMenu::None))).0;
    let mut toasts = use_context_provider(|| Signal::new(Vec::<Toast>::new()));
    let source_sync = use_context_provider(|| Signal::new(SourceSyncState::default()));
    let recovery_pending_at_launch = use_signal(|| has_pending_recovery(&state.read()));
    let recovery_dismissed = use_signal(|| false);

    // Surfaced once at startup, not per save (task 005 Part A): the
    // temp-directory fallback is now observable
    // (`AppPersistence::settings_used_temp_fallback`) rather than silently
    // discarded inside `unwrap_or_else`. Reads no signal — `persistence`
    // and `settings.lang` are plain values captured at this render, not
    // `Signal` reads — so this effect fires once on mount, not on every
    // re-render.
    let startup_persistence = persistence.clone();
    let startup_lang = settings.lang;
    use_effect(move || {
        if startup_persistence.settings_used_temp_fallback() {
            push_toast(
                &mut toasts,
                ToastKind::Warning,
                tr(startup_lang, "settings.temp_fallback_warning"),
            );
        }
    });

    // RFC-043: the most recent workspace was attempted at launch and
    // failed (unusable path). Non-blocking notice naming it, not a
    // panic or a silent Start Screen. `reopen_failure` is a plain value
    // already computed once above, so this also fires once on mount.
    use_effect(move || {
        if let Some(notice) = &reopen_failure {
            push_toast(
                &mut toasts,
                ToastKind::Warning,
                format!(
                    "{}: {}",
                    tr(startup_lang, "workspace.reopen_failed"),
                    notice.display_name
                ),
            );
        }
    });

    // Background: native fs watcher + autosave + external-change poll.
    use_future(move || {
        let mut app: Signal<AppState> = state;
        async move {
            let mut watcher: Option<(PathBuf, FsWatcher)> = None;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)).await;
                let ws_root = app.read().workspace.as_ref().map(|w| w.root_path.clone());
                match (&ws_root, watcher.as_ref().map(|(p, _)| p)) {
                    (Some(n), Some(c)) if n == c => {}
                    (Some(n), _) => {
                        watcher = FsWatcher::start(n).ok().map(|w| (n.clone(), w));
                    }
                    (None, _) => watcher = None,
                }
                if let Some((_, ref fw)) = watcher {
                    let events = fw.drain();
                    if !events.is_empty() {
                        let mut s = app.write();
                        let sess = s.session.as_ref().map(|ss| ss.path.clone());
                        let mut refresh = false;
                        for ev in events {
                            match ev {
                                WatchEvent::Modified(p) | WatchEvent::Deleted(p) => {
                                    if sess.as_ref() == Some(&p) {
                                        s.check_external_change();
                                    }
                                    refresh = true;
                                }
                                WatchEvent::Created(_) => refresh = true,
                            }
                        }
                        if refresh {
                            s.refresh_tree();
                        }
                    }
                }
                let mut s = app.write();
                if s.session.is_some() {
                    s.check_external_change();
                    let _ = s.autosave_tick(now_ms());
                }
            }
        }
    });

    // Global keyboard shortcut relay.
    let mode_sig = use_context::<Signal<EditorMode>>();
    let app_st: Signal<AppState> = state;
    let source_sync_for_shortcuts = source_sync;
    let toasts_for_shortcuts = use_context::<Signal<Vec<Toast>>>();
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        // Auto-restarting shortcut relay (RFC-002 hardening).
        let mut consecutive_failures = 0_u32;
        let mut generation = 0_u64;
        loop {
            generation = generation.saturating_add(1);
            let relay_js = bridge::relay_js("__bk_shortcut_relay", generation);
            let mut relay = document::eval(&relay_js);
            while let Ok(raw) = relay.recv().await {
                consecutive_failures = 0;
                if let Ok(AppMsg::Shortcut { key }) = serde_json::from_value(raw) {
                    match key.as_str() {
                        "save" => {
                            submit_source_command(
                                source_sync_for_shortcuts,
                                app_st,
                                mode_sig,
                                toasts_for_shortcuts,
                                SourceCommand::SaveNow,
                            );
                        }
                        "mode_text" => submit_source_shortcut_interaction(
                            source_sync_for_shortcuts,
                            app_st,
                            mode_sig,
                            toasts_for_shortcuts,
                            SourceCommand::SwitchMode(EditorMode::Text),
                        ),
                        "mode_form" => submit_source_shortcut_interaction(
                            source_sync_for_shortcuts,
                            app_st,
                            mode_sig,
                            toasts_for_shortcuts,
                            SourceCommand::SwitchMode(EditorMode::Form),
                        ),
                        "mode_preview" => submit_source_shortcut_interaction(
                            source_sync_for_shortcuts,
                            app_st,
                            mode_sig,
                            toasts_for_shortcuts,
                            SourceCommand::SwitchMode(EditorMode::Preview),
                        ),
                        "mode_split" => submit_source_shortcut_interaction(
                            source_sync_for_shortcuts,
                            app_st,
                            mode_sig,
                            toasts_for_shortcuts,
                            SourceCommand::SwitchMode(EditorMode::Split),
                        ),
                        _ => {}
                    }
                }
            }
            document::eval(&bridge::clear_relay_js("__bk_shortcut_relay", generation));
            consecutive_failures = consecutive_failures.saturating_add(1);
            let delay_ms = bridge::relay_restart_delay_ms(consecutive_failures);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    });

    let has_recovery = should_show_recovery(
        &state.read(),
        *recovery_pending_at_launch.read(),
        *recovery_dismissed.read(),
    );
    let workspace_open = state.read().workspace.is_some() || state.read().session.is_some();
    let settings_open = *use_context::<SettingsOpen>().0.read();
    bridge::trace(
        "app.render",
        format!(
            "has_recovery={has_recovery} workspace_open={workspace_open} settings_open={settings_open}"
        ),
    );

    rsx! {
        document::Style { "{STYLE_SOURCE}" }
        document::Script { "{SHORTCUTS_SOURCE}" }
        SourceEditorControllerHost {}
        if webview_smoke {
            WebViewSmokeDriver {}
        }
        if webview_key_spike {
            WebViewKeySpikeDriver {}
        }
        ToastLayer {}
        div {
            class: "app-frame",
            // Outside-click / focus-leaving-the-menu close path (RFC-042
            // §6.2 rule 3, §7.2): app_bar.rs's menu items call
            // `stop_propagation`, so this only fires for genuine
            // outside-click/focus-loss, never for in-menu interaction.
            // Release only — never restore here. Focus already moved
            // somewhere else (that's *why* this fired); forcing it back to
            // the trigger would fight that move, including the RFC-041
            // controller placing focus in CodeMirror (re-review C3).
            onclick: move |_| {
                release_menu_focus(source_sync, *open_menu.read());
                open_menu.set(OpenMenu::None);
            },
            onfocusin: move |_| {
                release_menu_focus(source_sync, *open_menu.read());
                open_menu.set(OpenMenu::None);
            },
            AppBar {}
            if settings_open {
                SettingsScreen {}
            } else if has_recovery {
                RecoveryScreen { dismissed: recovery_dismissed }
            } else if workspace_open {
                MainShell {}
            } else {
                StartScreen {}
            }
        }
    }
}

fn false_val() -> bool {
    false
}

pub(crate) fn has_pending_recovery(state: &AppState) -> bool {
    !state.recovery.list().is_empty()
}

pub(crate) fn should_show_recovery(
    state: &AppState,
    pending_at_launch: bool,
    dismissed: bool,
) -> bool {
    pending_at_launch && !dismissed && state.session.is_none() && has_pending_recovery(state)
}

#[component]
fn MainShell() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();
    let mode = *use_context::<Signal<EditorMode>>().read();
    let collapsed = *use_context::<ExplorerCollapsed>().0.read();
    let outline_open = use_context::<OutlineOpen>().0;
    let backlinks_open = use_context::<BacklinksOpen>().0;
    let history_open = use_context::<HistoryOpen>().0;
    let has_doc = state.read().session.is_some();
    bridge::trace(
        "main_shell.render",
        format!("mode={mode:?} has_doc={has_doc} collapsed={collapsed}"),
    );

    rsx! {
        div { class: "shell",
            if !collapsed { Explorer {} }
            main { class: "editor-pane",
                EditorHeader {}
                ConflictBanner {}
                div { class: "surface-row",
                    // Left: main editor surface
                    div { class: "surface",
                        if has_doc {
                            match mode {
                                EditorMode::Text    => rsx! { TextMode {} },
                                EditorMode::Form    => rsx! { FormMode {} },
                                EditorMode::Preview => rsx! { PreviewMode {} },
                                EditorMode::Split   => rsx! { SplitMode {} },
                            }
                        } else {
                            p { class: "empty-hint", {tr(lang, "editor.no_document")} }
                        }
                    }
                    // Right panels (mutually exclusive or stacked)
                    if *history_open.read() && has_doc {
                        HistoryPanel {}
                    } else if *backlinks_open.read() && has_doc {
                        BacklinksPanel {}
                    } else if *outline_open.read() && has_doc {
                        OutlinePanel {}
                    }
                }
                StatusBar {}
            }
        }
    }
}
