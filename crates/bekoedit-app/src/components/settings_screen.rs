//! Settings screen (RFC-022).

use dioxus::prelude::*;

use bekoedit_ui_contract::EditorMode;

use crate::i18n::{Lang, tr};
use crate::persistence::AppPersistence;
use crate::shell_focus;
use crate::source_sync::SourceSyncState;
use crate::state::SettingsOpen;

#[component]
pub fn SettingsScreen() -> Element {
    let mut settings_open = use_context::<SettingsOpen>().0;
    let mut lang_signal = use_context::<Signal<Lang>>();
    let mut mode_signal = use_context::<Signal<EditorMode>>();
    let persistence = use_context::<AppPersistence>();
    let mut source_sync = use_context::<Signal<SourceSyncState>>();
    let lang = *lang_signal.read();

    // Settings is a screen replacement (RFC-042 §7.5): exit releases the
    // shell authority acquired when Settings was opened and restores focus
    // to the app-bar menu trigger — the only persistent invoking control,
    // since the "Settings" menu item itself unmounts when its menu closes.
    // Any future Settings exit path (e.g. Escape, slice 4) must also route
    // through this closure — do not add a second, uncoordinated exit.
    let mut close_settings = move || {
        source_sync.write().release_shell_focus();
        shell_focus::focus_element(shell_focus::TRIGGER_APP_MENU);
        settings_open.set(false);
    };

    let mut settings = use_signal(|| persistence.load_settings());

    rsx! {
        div { class: "settings-screen",
            div { class: "settings-header",
                h1 { {tr(lang, "settings.title")} }
            }
            div { class: "settings-body",
                section { class: "settings-group",
                    h2 { {tr(lang, "settings.general")} }
                    label { class: "settings-row",
                        span { {tr(lang, "settings.language")} }
                        select {
                            onchange: move |evt| {
                                settings.write().lang = if evt.value() == "ja" { Lang::Ja } else { Lang::En };
                            },
                            option { value: "en", selected: settings.read().lang == Lang::En, "English" }
                            option { value: "ja", selected: settings.read().lang == Lang::Ja, "日本語" }
                        }
                    }
                    label { class: "settings-row",
                        span { {tr(lang, "settings.default_mode")} }
                        select {
                            onchange: move |evt| {
                                settings.write().default_mode = match evt.value().as_str() {
                                    "text"    => EditorMode::Text,
                                    "preview" => EditorMode::Preview,
                                    _         => EditorMode::Form,
                                };
                            },
                            option { value: "form",    selected: settings.read().default_mode == EditorMode::Form,    {tr(lang, "mode.form")} }
                            option { value: "text",    selected: settings.read().default_mode == EditorMode::Text,    {tr(lang, "mode.text")} }
                            option { value: "preview", selected: settings.read().default_mode == EditorMode::Preview, {tr(lang, "mode.preview")} }
                        }
                    }
                    label { class: "settings-row checkbox",
                        input {
                            r#type: "checkbox",
                            checked: settings.read().reopen_last_workspace,
                            onchange: move |evt| { settings.write().reopen_last_workspace = evt.checked(); },
                        }
                        span { {tr(lang, "settings.reopen")} }
                    }
                }
                section { class: "settings-group",
                    h2 { {tr(lang, "settings.editor")} }
                    label { class: "settings-row",
                        span { {tr(lang, "settings.autosave_ms")} }
                        input {
                            r#type: "number", min: "300", max: "10000", step: "100",
                            value: "{settings.read().core.autosave_debounce_ms}",
                            onchange: move |evt| {
                                if let Ok(ms) = evt.value().parse::<u64>() {
                                    settings.write().core.autosave_debounce_ms = ms.clamp(300, 10_000);
                                }
                            },
                        }
                        span { class: "muted", "ms" }
                    }
                    label { class: "settings-row checkbox",
                        input {
                            r#type: "checkbox",
                            checked: settings.read().core.prefer_trash,
                            onchange: move |evt| { settings.write().core.prefer_trash = evt.checked(); },
                        }
                        span { {tr(lang, "settings.prefer_trash")} }
                    }
                }
            }
            div { class: "settings-footer",
                button {
                    class: "primary",
                    onclick: move |_| {
                        let s = settings.read().clone();
                        lang_signal.set(s.lang);
                        mode_signal.set(s.default_mode);
                        persistence.save_settings(&s);
                        close_settings();
                    },
                    {tr(lang, "settings.save")}
                }
                button {
                    onclick: move |_| close_settings(),
                    {tr(lang, "settings.close")}
                }
            }
        }
    }
}
