//! Form Mode (RFC-016): the visual editing surface.
//!
//! Each block renders as a typed form control. Commits send semantic
//! `FormBlockEdit` commands (RFC-018) carrying revision-scoped block
//! identity — never byte ranges. Unsafe regions render as Raw Markdown
//! Islands (RFC-017) with an explanatory label and raw-text editing.
//!
//! Commits happen on `change` (blur/Enter), not per keystroke, so each
//! field interaction produces one minimal source patch.

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::fingerprint::BlockId;
use bekoedit_markdown::{FormBlockDisplay, FormBlockEdit, FormEditCommand, FormProjection};

use crate::i18n::{Lang, tr};
use crate::state::now_ms;

#[component]
pub fn FormMode() -> Element {
    let state = use_context::<Signal<AppState>>();
    let lang = *use_context::<Signal<Lang>>().read();

    let projection: Option<FormProjection> =
        state.read().session.as_ref().map(|s| s.form_projection());
    let Some(projection) = projection else {
        return rsx! { div {} };
    };
    let revision = projection.document_revision;

    rsx! {
        div { class: "form-mode",
            for block in projection.blocks {
                FormBlockView {
                    key: "{block.block_id.ordinal}-{block.block_id.fingerprint.content_hash}",
                    block_id: block.block_id,
                    display: block.display.clone(),
                    revision,
                    lang,
                }
            }
        }
    }
}

/// Builds and dispatches one semantic edit command.
fn dispatch(mut state: Signal<AppState>, revision: u64, block_id: BlockId, edit: FormBlockEdit) {
    let cmd = FormEditCommand {
        base_revision: revision,
        block_id,
        client_block_fingerprint: Some(block_id.fingerprint),
        edit,
    };
    let _ = state.write().edit_form(&cmd, now_ms());
}

#[component]
fn FormBlockView(
    block_id: BlockId,
    display: FormBlockDisplay,
    revision: u64,
    lang: Lang,
) -> Element {
    let state = use_context::<Signal<AppState>>();

    let body = match display {
        FormBlockDisplay::Heading {
            level,
            text,
            level_editable,
        } => rsx! {
            div { class: "block-row",
                select {
                    disabled: !level_editable,
                    onchange: move |evt| {
                        if let Ok(level) = evt.value().parse::<u8>() {
                            dispatch(state, revision, block_id, FormBlockEdit::SetHeadingLevel { level });
                        }
                    },
                    for l in 1u8..=6 {
                        option { value: "{l}", selected: l == level, "H{l}" }
                    }
                }
                input {
                    class: "heading-input level-{level}",
                    r#type: "text",
                    value: "{text}",
                    onchange: move |evt| dispatch(
                        state, revision, block_id,
                        FormBlockEdit::ReplacePlainText { text: evt.value() },
                    ),
                }
            }
        },
        FormBlockDisplay::Paragraph { text } | FormBlockDisplay::Blockquote { text } => rsx! {
            textarea {
                class: "paragraph-input",
                value: "{text}",
                onchange: move |evt| dispatch(
                    state, revision, block_id,
                    FormBlockEdit::ReplacePlainText { text: evt.value() },
                ),
            }
        },
        FormBlockDisplay::List { ordered, items } => rsx! {
            ul { class: if ordered { "list ordered" } else { "list" },
                for item in items {
                    li {
                        if let Some(checked) = item.task_checked {
                            input {
                                r#type: "checkbox",
                                checked,
                                onchange: {
                                    let ordinal = item.ordinal;
                                    move |evt: Event<FormData>| dispatch(
                                        state, revision, block_id,
                                        FormBlockEdit::ToggleTaskChecked {
                                            item_ordinal: ordinal,
                                            checked: evt.checked(),
                                        },
                                    )
                                },
                            }
                        }
                        input {
                            r#type: "text",
                            value: "{item.text}",
                            onchange: {
                                let ordinal = item.ordinal;
                                move |evt: Event<FormData>| dispatch(
                                    state, revision, block_id,
                                    FormBlockEdit::ReplaceListItemText {
                                        item_ordinal: ordinal,
                                        text: evt.value(),
                                    },
                                )
                            },
                        }
                    }
                }
            }
        },
        FormBlockDisplay::Code { language, code } => {
            let lang_value = language.unwrap_or_default();
            rsx! {
                div { class: "code-block",
                    label {
                        {tr(lang, "block.language")}
                        input {
                            r#type: "text",
                            class: "code-lang",
                            value: "{lang_value}",
                            onchange: {
                                let code = code.clone();
                                move |evt: Event<FormData>| dispatch(
                                    state, revision, block_id,
                                    FormBlockEdit::ReplaceCodeBlock {
                                        language: Some(evt.value()),
                                        code: code.clone(),
                                    },
                                )
                            },
                        }
                    }
                    textarea {
                        class: "code-input",
                        spellcheck: "false",
                        value: "{code}",
                        onchange: {
                            let lang_value = lang_value.clone();
                            move |evt: Event<FormData>| dispatch(
                                state, revision, block_id,
                                FormBlockEdit::ReplaceCodeBlock {
                                    language: Some(lang_value.clone()),
                                    code: evt.value(),
                                },
                            )
                        },
                    }
                }
            }
        }
        FormBlockDisplay::HorizontalRule => rsx! { hr {} },
        FormBlockDisplay::RawIsland {
            label_key,
            text,
            editable,
            ..
        } => rsx! {
            div { class: "raw-island",
                div { class: "island-header",
                    span { class: "island-label", {tr(lang, &label_key)} }
                    span { class: "island-hint", {tr(lang, "island.hint")} }
                }
                textarea {
                    class: "island-input",
                    spellcheck: "false",
                    readonly: !editable,
                    value: "{text}",
                    onchange: move |evt| dispatch(
                        state, revision, block_id,
                        FormBlockEdit::ReplaceRawIsland { text: evt.value() },
                    ),
                }
            }
        },
    };

    rsx! {
        section { class: "form-block",
            {body}
            button {
                class: "block-delete",
                title: tr(lang, "block.delete"),
                onclick: move |_| dispatch(state, revision, block_id, FormBlockEdit::DeleteBlock),
                "×"
            }
        }
    }
}
