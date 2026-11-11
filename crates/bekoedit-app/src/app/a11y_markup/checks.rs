//! The §3.4 table of RFC-050 slice 1's handoff, as headless tests (level 2).
//! Every test runs for both languages. A mutation that removes a name, a role,
//! or a live attribute, or that renders English in the Japanese page, fails a
//! test here by name.

use bekoedit_ui_contract::EditorMode;

use super::harness::{Opts, Page, Screen, render};
use crate::i18n::{Lang, tr};
use crate::state::OpenMenu;

const LANGS: [Lang; 2] = [Lang::En, Lang::Ja];
const MODES: [EditorMode; 4] = [
    EditorMode::Text,
    EditorMode::Form,
    EditorMode::Preview,
    EditorMode::Split,
];
/// The roles a user operates; each needs an accessible name.
const OPERABLE: &[&str] = &[
    "button",
    "textbox",
    "combobox",
    "checkbox",
    "spinbutton",
    "tab",
    "treeitem",
    "menuitem",
    "link",
];

fn doc(lang: Lang, mode: EditorMode) -> Page {
    render(lang, mode, Screen::Document, Opts::default())
}

/// What `find` could not find, with the page's roles and names, so that a failure
/// says what was there.
fn missing(page: &Page, lang: Lang, role: &str, key: &str) -> String {
    format!(
        "{lang:?}: no {role} named {:?} ({key}). Same role on the page: {:?}. All: {}",
        tr(lang, key),
        page.names_with_role(role),
        page.describe()
    )
}

fn expect_named(page: &Page, lang: Lang, role: &str, key: &str) {
    assert!(
        page.find(role, tr(lang, key)).is_some(),
        "{}",
        missing(page, lang, role, key)
    );
}

/// An accessible name is no name if it is only a symbol: it must hold a letter or digit.
fn is_speakable(name: &str) -> bool {
    name.chars().any(char::is_alphanumeric)
}

#[test]
fn every_operable_control_has_a_speakable_name_on_every_screen_in_both_languages() {
    let screens = [
        (Screen::Start, EditorMode::Text),
        (Screen::NoDocument, EditorMode::Text),
        (Screen::Document, EditorMode::Text),
        (Screen::Document, EditorMode::Form),
        (Screen::Document, EditorMode::Preview),
        (Screen::Document, EditorMode::Split),
        (Screen::Settings, EditorMode::Text),
    ];
    for lang in LANGS {
        for (screen, mode) in screens {
            for menu in [OpenMenu::None, OpenMenu::App, OpenMenu::EditorTools] {
                let page = render(
                    lang,
                    mode,
                    screen,
                    Opts {
                        toast: true,
                        menu,
                        ..Opts::default()
                    },
                );
                assert!(
                    !page.all().is_empty(),
                    "{lang:?} {screen:?} {mode:?}: rendered nothing"
                );
                for role in OPERABLE {
                    for element in page.with_role(role) {
                        let name = page.name_of(element);
                        assert!(
                            name.as_deref().is_some_and(is_speakable),
                            "{lang:?} {screen:?} {mode:?} menu {menu:?}: a {role} (<{} class={:?}>) has name {name:?}",
                            element.tag,
                            element.attr("class"),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_shell_has_its_regions_and_controls_by_name() {
    for lang in LANGS {
        for mode in MODES {
            let page = doc(lang, mode);
            for (role, key) in [
                ("complementary", "explorer.label"),
                ("tree", "explorer.tree_label"),
                ("button", "explorer.new_file"),
                ("button", "search.label"),
                ("button", "explorer.toggle"),
                ("toolbar", "editor.toolbar_label"),
                ("button", "menu.app"),
                ("button", "menu.editor_tools"),
                ("button", "editor.save"),
            ] {
                expect_named(&page, lang, role, key);
            }
        }
    }
}

#[test]
fn the_three_mode_tabs_are_tabs_with_exactly_one_selected_and_it_is_the_right_one() {
    let tabs = [
        (EditorMode::Text, "mode.text"),
        (EditorMode::Form, "mode.form"),
        (EditorMode::Preview, "mode.preview"),
    ];
    for lang in LANGS {
        for (mode, _) in tabs {
            let page = doc(lang, mode);
            expect_named(&page, lang, "tablist", "editor.mode_label");
            let selected: Vec<String> = page
                .with_role("tab")
                .into_iter()
                .filter(|t| t.attr("aria-selected") == Some("true"))
                .map(|t| page.name_of(t).unwrap_or_default())
                .collect();
            let expected = tabs
                .iter()
                .find(|(m, _)| *m == mode)
                .map(|(_, key)| tr(lang, key))
                .unwrap();
            assert_eq!(
                selected,
                [expected.to_string()],
                "{lang:?} {mode:?}: the selected tab(s)"
            );
            for (_, key) in tabs {
                let tab = page
                    .find("tab", tr(lang, key))
                    .unwrap_or_else(|| panic!("{}", missing(&page, lang, "tab", key)));
                assert!(
                    matches!(tab.attr("aria-selected"), Some("true") | Some("false")),
                    "{lang:?} {mode:?}: tab {key} has no aria-selected"
                );
            }
        }
    }
}

#[test]
fn the_save_control_is_a_button_and_the_status_reads_as_a_save_string() {
    let states = [
        "save.clean",
        "save.dirty",
        "save.scheduled",
        "save.saving",
        "save.saved",
        "save.failed",
        "save.conflict",
    ];
    for lang in LANGS {
        for dirty in [false, true] {
            let page = render(
                lang,
                EditorMode::Text,
                Screen::Document,
                Opts {
                    dirty,
                    ..Opts::default()
                },
            );
            expect_named(&page, lang, "button", "editor.save");
            let strings: Vec<&str> = states.iter().map(|k| tr(lang, k)).collect();
            let status: Vec<_> = page
                .all()
                .into_iter()
                .filter(|e| e.has_class("save-state"))
                .collect();
            assert_eq!(
                status.len(),
                1,
                "{lang:?}: one save state in the footer, found {}",
                status.len()
            );
            assert_eq!(
                status[0].role().as_deref(),
                Some("status"),
                "{lang:?}: the footer's save state is a status"
            );
            let text = status[0]
                .children
                .iter()
                .map(|c| format!("{c:?}"))
                .collect::<String>();
            assert!(
                strings.iter().any(|s| text.contains(s)),
                "{lang:?}: status {text} is none of {strings:?}"
            );
            let expected = if dirty {
                [tr(lang, "save.dirty"), tr(lang, "save.scheduled")]
            } else {
                [tr(lang, "save.clean"); 2]
            };
            assert!(
                expected.iter().any(|s| text.contains(s)),
                "{lang:?} dirty={dirty}: status {text}, expected one of {expected:?}"
            );
            let live: Vec<_> = page
                .all()
                .into_iter()
                .filter(|e| e.has_class("save-status"))
                .collect();
            assert_eq!(live.len(), 1, "{lang:?}: the header's save status");
            assert_eq!(
                live[0].attr("aria-live"),
                Some("polite"),
                "{lang:?}: the header's save status is a polite live region"
            );
        }
    }
}

#[test]
fn the_notices_area_is_a_live_region_even_when_empty() {
    for lang in LANGS {
        for toast in [false, true] {
            for (screen, mode) in [
                (Screen::Start, EditorMode::Text),
                (Screen::Document, EditorMode::Text),
                (Screen::Settings, EditorMode::Text),
            ] {
                let page = render(
                    lang,
                    mode,
                    screen,
                    Opts {
                        toast,
                        ..Opts::default()
                    },
                );
                let layer: Vec<_> = page
                    .all()
                    .into_iter()
                    .filter(|e| e.has_class("toast-layer"))
                    .collect();
                assert_eq!(
                    layer.len(),
                    1,
                    "{lang:?} {screen:?} toast={toast}: the notices area is missing from the page"
                );
                assert_eq!(
                    layer[0].attr("role"),
                    Some("status"),
                    "{lang:?}: the notices area has no status role"
                );
                assert_eq!(
                    layer[0].attr("aria-live"),
                    Some("polite"),
                    "{lang:?}: the notices area is not a polite live region"
                );
                if toast {
                    expect_named(&page, lang, "button", "toast.dismiss");
                }
            }
        }
    }
}

#[test]
fn the_overflow_menus_are_menus_with_named_items_and_the_triggers_say_so() {
    for lang in LANGS {
        let closed = doc(lang, EditorMode::Text);
        for key in ["menu.app", "menu.editor_tools"] {
            let trigger = closed
                .find("button", tr(lang, key))
                .unwrap_or_else(|| panic!("{}", missing(&closed, lang, "button", key)));
            assert_eq!(
                trigger.attr("aria-haspopup"),
                Some("menu"),
                "{lang:?} {key}"
            );
            assert_eq!(
                trigger.attr("aria-expanded"),
                Some("false"),
                "{lang:?} {key} closed"
            );
        }
        let app = render(
            lang,
            EditorMode::Text,
            Screen::Document,
            Opts {
                menu: OpenMenu::App,
                ..Opts::default()
            },
        );
        assert_eq!(
            app.find("button", tr(lang, "menu.app"))
                .and_then(|t| t.attr("aria-expanded")),
            Some("true"),
            "{lang:?}"
        );
        assert!(
            !app.with_role("menu").is_empty(),
            "{lang:?}: the open app menu has role menu"
        );
        // The control that opens Settings.
        expect_named(&app, lang, "menuitem", "settings.title");
        let tools = render(
            lang,
            EditorMode::Text,
            Screen::Document,
            Opts {
                menu: OpenMenu::EditorTools,
                ..Opts::default()
            },
        );
        assert!(
            !tools.with_role("menu").is_empty(),
            "{lang:?}: the open tools menu has role menu"
        );
        assert!(
            !tools.with_role("menuitem").is_empty(),
            "{lang:?}: the tools menu has items"
        );
    }
}

#[test]
fn settings_is_a_named_region_with_its_controls_named() {
    for lang in LANGS {
        let page = render(lang, EditorMode::Text, Screen::Settings, Opts::default());
        expect_named(&page, lang, "region", "settings.title");
        expect_named(&page, lang, "combobox", "settings.language");
        expect_named(&page, lang, "combobox", "settings.default_mode");
        expect_named(&page, lang, "checkbox", "settings.reopen");
        expect_named(&page, lang, "checkbox", "settings.prefer_trash");
        expect_named(&page, lang, "spinbutton", "settings.autosave_ms");
        expect_named(&page, lang, "button", "settings.save");
        expect_named(&page, lang, "button", "settings.close");
    }
}

#[test]
fn the_start_screen_renders_headless_and_its_actions_are_named() {
    for lang in LANGS {
        let page = render(lang, EditorMode::Text, Screen::Start, Opts::default());
        assert!(
            page.with_role("button").len() >= 2,
            "{lang:?}: the Start Screen's buttons: {}",
            page.describe()
        );
        expect_named(&page, lang, "button", "menu.app");
    }
}

#[test]
fn without_a_document_the_shell_has_no_mode_tabs_and_says_so() {
    for lang in LANGS {
        let page = render(lang, EditorMode::Text, Screen::NoDocument, Opts::default());
        assert!(
            page.with_role("tab").is_empty(),
            "{lang:?}: tabs without a document"
        );
        expect_named(&page, lang, "complementary", "explorer.label");
    }
}

#[test]
fn form_mode_has_one_named_field_for_each_kind_of_block_and_its_history_buttons() {
    for lang in LANGS {
        let page = doc(lang, EditorMode::Form);
        // Every block is a named group.
        for kind in [
            "block.kind.heading",
            "block.kind.paragraph",
            "block.kind.blockquote",
            "block.kind.list",
            "block.kind.code",
            "block.kind.table",
        ] {
            expect_named(&page, lang, "group", kind);
        }
        // The fields in them.
        expect_named(&page, lang, "textbox", "block.kind.heading");
        expect_named(&page, lang, "combobox", "block.heading_level");
        expect_named(&page, lang, "textbox", "block.kind.paragraph");
        expect_named(&page, lang, "textbox", "block.kind.blockquote");
        expect_named(&page, lang, "textbox", "block.list_item");
        expect_named(&page, lang, "checkbox", "block.task_item");
        expect_named(&page, lang, "textbox", "block.kind.code");
        // Every block can be deleted by name, not by its "×".
        assert!(
            page.with_role("button")
                .iter()
                .filter(|b| page.name_of(b).as_deref() == Some(tr(lang, "block.delete")))
                .count()
                >= 6,
            "{lang:?}: each block's delete button is named {:?}: {}",
            tr(lang, "block.delete"),
            page.describe()
        );
        // History.
        expect_named(&page, lang, "button", "form.undo");
        expect_named(&page, lang, "button", "form.redo");
        expect_named(&page, lang, "toolbar", "form.history.label");
        // The table: a table with column headers and named cells.
        assert_eq!(page.with_role("table").len(), 1, "{lang:?}: the table");
        assert!(
            !page.with_role("columnheader").is_empty(),
            "{lang:?}: the table has column headers"
        );
        let cells = page
            .all()
            .into_iter()
            .filter(|e| e.has_class("table-cell-input"))
            .count();
        assert!(cells >= 4, "{lang:?}: table cell inputs: {cells}");
    }
}
