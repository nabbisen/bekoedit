//! Per-block form view components (RFC-016/017/027/028).

use dioxus::prelude::*;

use bekoedit_core::AppState;
use bekoedit_markdown::{FormBlockDisplay, FormBlockEdit, fingerprint::BlockId};

use super::dispatch;
use super::inline_toolbar::InlineToolbar;
use crate::i18n::{Lang, tr};

// ─── Per-block view ───────────────────────────────────────────────────────────

#[component]
pub fn FormBlockView(
    block_id: BlockId,
    display: FormBlockDisplay,
    revision: u64,
    lang: Lang,
) -> Element {
    let state = use_context::<Signal<AppState>>();
    let field_id = format!(
        "fb-{}-{}",
        block_id.ordinal, block_id.fingerprint.content_hash
    );

    let body = match display {
        // ── Heading ──────────────────────────────────────────────────────────
        FormBlockDisplay::Heading {
            level,
            text,
            level_editable,
        } => rsx! {
            div { class: "block-row",
                select {
                    disabled: !level_editable,
                    onchange: move |evt| {
                        if let Ok(l) = evt.value().parse::<u8>() {
                            dispatch(state, revision, block_id,
                                     FormBlockEdit::SetHeadingLevel { level: l });
                        }
                    },
                    for l in 1u8..=6 {
                        option { value: "{l}", selected: l == level, "H{l}" }
                    }
                }
                input {
                    id: "{field_id}",
                    class: "heading-input level-{level}",
                    r#type: "text",
                    value: "{text}",
                    onchange: move |evt| dispatch(state, revision, block_id,
                        FormBlockEdit::ReplacePlainText { text: evt.value() }),
                }
            }
        },

        // ── Paragraph ────────────────────────────────────────────────────────
        FormBlockDisplay::Paragraph { text } => rsx! {
            InlineToolbar { field_id: field_id.clone(), block_id, revision, lang }
            textarea {
                id: "{field_id}",
                class: "paragraph-input",
                value: "{text}",
                onchange: move |evt| dispatch(state, revision, block_id,
                    FormBlockEdit::ReplacePlainText { text: evt.value() }),
            }
        },

        // ── Blockquote ───────────────────────────────────────────────────────
        FormBlockDisplay::Blockquote { text } => rsx! {
            InlineToolbar { field_id: field_id.clone(), block_id, revision, lang }
            textarea {
                id: "{field_id}",
                class: "paragraph-input blockquote-input",
                value: "{text}",
                onchange: move |evt| dispatch(state, revision, block_id,
                    FormBlockEdit::ReplacePlainText { text: evt.value() }),
            }
        },

        // ── List ─────────────────────────────────────────────────────────────
        FormBlockDisplay::List { ordered, items } => rsx! {
            ul { class: if ordered { "list ordered" } else { "list" },
                for item in items {
                    li {
                        if let Some(checked) = item.task_checked {
                            input {
                                r#type: "checkbox",
                                checked,
                                onchange: {
                                    let ord = item.ordinal;
                                    move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                        FormBlockEdit::ToggleTaskChecked {
                                            item_ordinal: ord,
                                            checked: evt.checked(),
                                        })
                                },
                            }
                        }
                        input {
                            r#type: "text",
                            value: "{item.text}",
                            onchange: {
                                let ord = item.ordinal;
                                move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                    FormBlockEdit::ReplaceListItemText {
                                        item_ordinal: ord,
                                        text: evt.value(),
                                    })
                            },
                        }
                    }
                }
            }
        },

        // ── Code block ───────────────────────────────────────────────────────
        FormBlockDisplay::Code { language, code } => {
            let lang_val = language.unwrap_or_default();
            rsx! {
                div { class: "code-block",
                    label {
                        {tr(lang, "block.language")}
                        input {
                            r#type: "text",
                            class: "code-lang",
                            value: "{lang_val}",
                            onchange: {
                                let code = code.clone();
                                move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                    FormBlockEdit::ReplaceCodeBlock {
                                        language: Some(evt.value()),
                                        code: code.clone(),
                                    })
                            },
                        }
                    }
                    textarea {
                        class: "code-input",
                        spellcheck: "false",
                        value: "{code}",
                        onchange: {
                            let lv = lang_val.clone();
                            move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                FormBlockEdit::ReplaceCodeBlock {
                                    language: Some(lv.clone()),
                                    code: evt.value(),
                                })
                        },
                    }
                }
            }
        }

        // ── Horizontal rule ──────────────────────────────────────────────────
        FormBlockDisplay::HorizontalRule => rsx! { hr {} },

        // ── Simple table (RFC-027) ────────────────────────────────────────────
        FormBlockDisplay::Table {
            headers,
            rows,
            col_count,
        } => rsx! {
            div { class: "table-block",
                table {
                    thead {
                        tr {
                            for (ci, header) in headers.iter().enumerate() {
                                th {
                                    input {
                                        r#type: "text",
                                        class: "table-cell-input",
                                        value: "{header}",
                                        onchange: {
                                                    move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                                FormBlockEdit::ReplaceTableCell {
                                                    row: 0, col: ci, text: evt.value(),
                                                })
                                        },
                                    }
                                }
                            }
                        }
                    }
                    tbody {
                        for (ri, row) in rows.iter().enumerate() {
                            tr {
                                for (ci, cell) in row.iter().enumerate() {
                                    td {
                                        input {
                                            r#type: "text",
                                            class: "table-cell-input",
                                            value: "{cell}",
                                            onchange: {
                                                let ri = ri + 1; // 0=header
                                                            move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                                    FormBlockEdit::ReplaceTableCell {
                                                        row: ri, col: ci, text: evt.value(),
                                                    })
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                button {
                    class: "table-add-row",
                    onclick: move |_| dispatch(state, revision, block_id, FormBlockEdit::AddTableRow),
                    {tr(lang, "table.add_row")}
                }
                // Warn if col_count == 0 (degenerate table).
                if col_count == 0 {
                    p { class: "muted", {tr(lang, "table.empty")} }
                }
            }
        },

        // ── Image card (RFC-028) ──────────────────────────────────────────────
        FormBlockDisplay::Image { alt, src } => rsx! {
            div { class: "image-card",
                if !src.is_empty() {
                    img { class: "image-preview", src: "{src}", alt: "{alt}",
                          loading: "lazy",
                          style: "max-width:100%;max-height:200px" }
                }
                label { class: "settings-row",
                    span { {tr(lang, "image.alt")} }
                    input {
                        r#type: "text",
                        value: "{alt}",
                        onchange: {
                            let s = src.clone();
                            move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                FormBlockEdit::ReplaceImage { alt: evt.value(), src: s.clone() })
                        },
                    }
                }
                label { class: "settings-row",
                    span { {tr(lang, "image.src")} }
                    input {
                        r#type: "text",
                        value: "{src}",
                        onchange: {
                            let a = alt.clone();
                            move |evt: Event<FormData>| dispatch(state, revision, block_id,
                                FormBlockEdit::ReplaceImage { alt: a.clone(), src: evt.value() })
                        },
                    }
                }
            }
        },

        // ── Raw Markdown Island ───────────────────────────────────────────────
        FormBlockDisplay::RawIsland {
            label_key,
            text,
            editable,
            ..
        } => rsx! {
            div { class: "raw-island",
                div { class: "island-header",
                    span { class: "island-label", {tr(lang, &label_key)} }
                    span { class: "island-hint",  {tr(lang, "island.hint")} }
                }
                textarea {
                    class: "island-input",
                    spellcheck: "false",
                    readonly: !editable,
                    value: "{text}",
                    onchange: move |evt| dispatch(state, revision, block_id,
                        FormBlockEdit::ReplaceRawIsland { text: evt.value() }),
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
