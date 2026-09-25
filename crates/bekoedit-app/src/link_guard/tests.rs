// Link guard effects (task 032). `decide_link_click` has its own tests in
// bekoedit-core; these pin what the app does with each decision, and that
// nothing else in the crate can reach the OS opener.

use std::path::PathBuf;

use bekoedit_core::{LinkAction, LinkRefusal, decide_link_click};

use super::{GuardMessage, LINK_GUARD_JS, LinkEffect, link_effect, parse_guard_message};
use crate::i18n::{Lang, tr};

const EVERY_REFUSAL: [LinkRefusal; 9] = [
    LinkRefusal::NoTarget,
    LinkRefusal::UnsupportedScheme,
    LinkRefusal::NetworkPath,
    LinkRefusal::AbsolutePath,
    LinkRefusal::InvalidPath,
    LinkRefusal::NoBase,
    LinkRefusal::OutsideWorkspace,
    LinkRefusal::NotFound,
    LinkRefusal::NotADocument,
];

#[test]
fn only_an_external_decision_becomes_an_open() {
    let external = decide_link_click("https://example.com/x", None, None);
    assert!(
        matches!(link_effect(external), LinkEffect::Open(url) if url.as_str() == "https://example.com/x")
    );

    let mut others = vec![
        LinkAction::InPage("top".into()),
        LinkAction::OpenDocument(PathBuf::from("a.md")),
    ];
    others.extend(EVERY_REFUSAL.into_iter().map(LinkAction::Refuse));
    for action in others {
        let effect = link_effect(action.clone());
        assert!(
            !matches!(effect, LinkEffect::Open(_)),
            "{action:?} must not open anything, got {effect:?}"
        );
    }
}

#[test]
fn a_fragment_click_does_nothing_outside_the_page_and_says_nothing() {
    assert_eq!(
        link_effect(LinkAction::InPage("section".into())),
        LinkEffect::Nothing
    );
}

#[test]
fn every_link_that_is_not_followed_says_so_in_both_languages() {
    let mut keys: Vec<&'static str> = EVERY_REFUSAL
        .into_iter()
        .map(|reason| match link_effect(LinkAction::Refuse(reason)) {
            LinkEffect::Notice(key) => key,
            other => panic!("{reason:?} must give a notice, got {other:?}"),
        })
        .collect();
    match link_effect(LinkAction::OpenDocument(PathBuf::from("a.md"))) {
        LinkEffect::Notice(key) => keys.push(key),
        other => panic!("a document link must give a notice, got {other:?}"),
    }
    for key in &keys {
        assert!(!tr(Lang::En, key).is_empty(), "EN {key}");
        assert!(!tr(Lang::Ja, key).is_empty(), "JA {key}");
    }
    let mut distinct = keys.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        keys.len(),
        "each reason has its own wording"
    );
    assert!(!tr(Lang::En, "link.open_failed").is_empty());
    assert!(!tr(Lang::Ja, "link.open_failed").is_empty());
}

/// `webbrowser` is the OS opener. Only `link_guard.rs` may name it, and it may
/// name it once, so the one call is the one that takes an `ExternalUrl`.
#[test]
fn the_os_opener_is_called_from_one_place() {
    fn visit(dir: &std::path::Path, needle: &str, hits: &mut Vec<(PathBuf, usize)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, needle, hits);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let calls = text
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("//"))
                    .filter(|line| line.contains(needle))
                    .count();
                if calls > 0 {
                    hits.push((path, calls));
                }
            }
        }
    }
    // Built at run time, so this file does not match itself.
    let needle = ["web", "browser::"].concat();
    let mut hits = Vec::new();
    visit(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &needle,
        &mut hits,
    );
    let names: Vec<_> = hits
        .iter()
        .map(|(path, calls)| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                *calls,
            )
        })
        .collect();
    assert_eq!(names, vec![("link_guard.rs".to_string(), 1)], "{hits:?}");
}

#[test]
fn the_script_cancels_before_dioxus_can_and_reports_only_clicks() {
    assert!(LINK_GUARD_JS.contains("addEventListener(\"click\", guard, true)"));
    assert!(LINK_GUARD_JS.contains("addEventListener(\"auxclick\", guard, true)"));
    assert!(LINK_GUARD_JS.contains("event.stopPropagation()"));
    assert!(LINK_GUARD_JS.contains("dioxus.send("));
    // The second layer: the interpreter's own route is switched off.
    assert!(LINK_GUARD_JS.contains("interpreter.intercept_link_redirects = false"));
}

#[test]
fn the_scripts_two_messages_are_read_and_anything_else_is_ignored() {
    use serde_json::json;
    assert_eq!(
        parse_guard_message(json!({"kind": "click", "href": "other.md"})),
        Some(GuardMessage::Click {
            href: "other.md".into()
        })
    );
    assert_eq!(
        parse_guard_message(json!({"kind": "trace", "detail": "x"})),
        Some(GuardMessage::Trace { detail: "x".into() })
    );
    // A bare string is the old shape; it must not be taken for a click.
    for other in [
        json!("other.md"),
        json!(null),
        json!({"kind": "click"}),
        json!({"kind": "click", "href": 3}),
        json!({"kind": "open", "href": "x"}),
        json!({}),
    ] {
        assert_eq!(parse_guard_message(other.clone()), None, "{other}");
    }
}
