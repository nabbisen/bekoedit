//! The Text/Preview/Form mode tablist (RFC-042 §7.3, handoff §5.5).
//!
//! Roving tabindex bound to the *selected* mode, manual activation only —
//! extracted from `editor_header.rs` to stay under the ELOC guideline
//! (handoff §11 risk table).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_ui_contract::EditorMode;

use crate::components::toast::Toast;
use crate::i18n::{Lang, tr};
use crate::shell_focus;
use crate::source_sync::{
    SourceCommand, SourceInteractionOrigin, SourceSyncState, submit_source_interaction,
};

#[derive(Props, Clone, PartialEq)]
pub(super) struct ModeTabsProps {
    pub(super) mode: EditorMode,
    pub(super) ui_lang: Lang,
    pub(super) state: Signal<AppState>,
    pub(super) mode_sig: Signal<EditorMode>,
    pub(super) source_sync: Signal<SourceSyncState>,
    pub(super) toasts: Signal<Vec<Toast>>,
}

#[component]
pub(super) fn ModeTabs(props: ModeTabsProps) -> Element {
    let ModeTabsProps {
        mode,
        ui_lang,
        state,
        mode_sig,
        source_sync,
        toasts,
    } = props;

    rsx! {
        nav {
            id: shell_focus::TABLIST_MODE_SWITCH,
            class: "mode-switch",
            role: "tablist",
            aria_label: tr(ui_lang, "editor.mode_label"),
            // Arrow/Home/End move DOM focus only — a pure query against the
            // rendered tabs, exactly like the tree (slice 2) and menus (this
            // slice, §5.3). Deliberately does not touch source_sync in any
            // way: RFC-042 §7.3 requires manual activation, and the tablist
            // is persistent UI, not a focus-owning surface under §6.3
            // (handoff §5.6) — Enter/Space still reach each tab's own
            // `onclick` via native button-activation semantics.
            onkeydown: move |event: KeyboardEvent| {
                if let Some(target) = shell_focus::tab_key_intent(&event.key()) {
                    event.prevent_default();
                    shell_focus::focus_tab(target);
                }
            },
            for (m, key) in [
                (EditorMode::Text,    "mode.text"),
                (EditorMode::Preview, "mode.preview"),
            ] {
                button {
                    class: if mode == m { "mode-tab active" } else { "mode-tab" },
                    "data-source-focus-launch": if m == EditorMode::Text { "mode-text" } else { "mode-preview" },
                    role: "tab",
                    tabindex: if mode == m { "0" } else { "-1" },
                    aria_selected: "{mode == m}",
                    onclick: move |_| {
                        submit_source_interaction(
                            source_sync,
                            state,
                            mode_sig,
                            toasts,
                            SourceCommand::SwitchMode(m),
                            SourceInteractionOrigin::persistent_control(
                                if m == EditorMode::Text { "mode-text" } else { "mode-preview" },
                            ),
                            || {},
                        );
                    },
                    {tr(ui_lang, key)}
                }
            }
            // Form Mode — still in the primary bar but AFTER Text/Preview
            button {
                class: if mode == EditorMode::Form { "mode-tab active" } else { "mode-tab mode-tab-secondary" },
                "data-source-focus-launch": "mode-form",
                role: "tab",
                tabindex: if mode == EditorMode::Form { "0" } else { "-1" },
                aria_selected: "{mode == EditorMode::Form}",
                onclick: move |_| {
                    submit_source_interaction(
                        source_sync,
                        state,
                        mode_sig,
                        toasts,
                        SourceCommand::SwitchMode(EditorMode::Form),
                        SourceInteractionOrigin::persistent_control("mode-form"),
                        || {},
                    );
                },
                {tr(ui_lang, "mode.form")}
            }
        }
    }
}
