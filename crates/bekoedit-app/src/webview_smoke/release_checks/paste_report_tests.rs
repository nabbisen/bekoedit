// The report half of the paste probe (RFC-046 slice 2, part A): what it says about
// each shape of observation, and that the page's field names and Rust's agree.

use super::paste_report::{Case, Constructed, Helper, Observation, ProbeEvent, Taken, answers};

fn key(kind: &str, shift: bool) -> ProbeEvent {
    ProbeEvent {
        kind: kind.into(),
        key: Some(if shift { "V" } else { "v" }.into()),
        ctrl: Some(true),
        shift: Some(shift),
        ..Default::default()
    }
}

fn paste(types: &[&str], html: usize, plain: usize) -> ProbeEvent {
    ProbeEvent {
        kind: "paste".into(),
        has_clipboard_data: Some(true),
        types: Some(types.iter().map(|t| t.to_string()).collect()),
        html_length: Some(html),
        html_head: Some("<h1>Probe".into()),
        plain_length: Some(plain),
        plain_head: Some("Probe heading".into()),
        target: Some("cm-content".into()),
        trusted: true,
        ..Default::default()
    }
}

fn case(events: Vec<ProbeEvent>, before: &str, after: &str) -> Case {
    Case {
        events,
        doc_before: Some(before.into()),
        doc_after: Some(after.into()),
    }
}

fn ready() -> Helper {
    Helper {
        ready: true,
        note: String::new(),
    }
}

fn both_flavours_case() -> Case {
    case(
        vec![
            key("keydown", false),
            paste(&["text/html", "text/plain"], 63, 40),
            key("keyup", false),
        ],
        "abc",
        "abcPasted",
    )
}

fn text(observation: &Observation) -> String {
    answers(observation).join("\n")
}

fn verdict(observation: &Observation, number: u8) -> String {
    answers(observation)
        .into_iter()
        .find(|line| line.starts_with(&format!("VERDICT {number}:")))
        .unwrap_or_else(|| panic!("no VERDICT {number} line"))
}

#[test]
fn a_real_paste_with_both_flavours_is_verdict_one_yes_and_says_what_it_carried() {
    let observation = Observation {
        helper: ready(),
        ctrl_v: both_flavours_case(),
        ..Default::default()
    };
    assert!(
        verdict(&observation, 1).ends_with("YES"),
        "{}",
        verdict(&observation, 1)
    );
    let all = text(&observation);
    assert!(
        all.contains("text/html 63 chars") && all.contains("text/plain 40 chars"),
        "{all}"
    );
    assert!(
        all.contains("the editor text changed, 3 to 9 characters"),
        "{all}"
    );
    assert!(
        all.contains("Q-order Ctrl+V: the keydown came BEFORE the paste event"),
        "{all}"
    );
    assert!(
        all.contains("holds the selection with text/html and text/plain"),
        "{all}"
    );
}

#[test]
fn html_alone_or_an_empty_flavour_is_not_both() {
    for event in [
        paste(&["text/html"], 63, 0),
        paste(&["text/plain"], 0, 40),
        paste(&["text/html", "text/plain"], 0, 40),
        paste(&["text/html", "text/plain"], 63, 0),
        paste(&[], 0, 0),
    ] {
        let observation = Observation {
            ctrl_v: case(vec![event.clone()], "a", "a"),
            ..Default::default()
        };
        assert!(verdict(&observation, 1).ends_with("NO"), "{event:?}");
    }
}

#[test]
fn no_paste_event_at_all_is_said_plainly_with_the_events_that_did_arrive() {
    let observation = Observation {
        ctrl_v: case(vec![key("keydown", false), key("keyup", false)], "a", "a"),
        ..Default::default()
    };
    let all = text(&observation);
    assert!(all.contains("no paste event reached the page"), "{all}");
    assert!(all.contains("keydown Ctrl+v > keyup Ctrl+v"), "{all}");
    assert!(all.contains("the editor text did not change"), "{all}");
    assert!(verdict(&observation, 1).ends_with("NO"));

    let silent = Observation::default();
    assert!(text(&silent).contains("no events at all"));
}

#[test]
fn the_chord_verdict_separates_the_paste_event_from_its_keydown() {
    let only_keydown = Observation {
        chord: case(vec![key("keydown", true)], "a", "a"),
        ..Default::default()
    };
    let line = verdict(&only_keydown, 2);
    assert!(
        line.contains("dispatches a paste event at all: NO"),
        "{line}"
    );
    assert!(line.contains("its keydown is visible: YES"), "{line}");

    let both = Observation {
        chord: case(
            vec![
                key("keydown", true),
                paste(&["text/html", "text/plain"], 5, 5),
            ],
            "a",
            "ab",
        ),
        ..Default::default()
    };
    let line = verdict(&both, 2);
    assert!(
        line.contains("at all: YES") && line.contains("keydown is visible: YES"),
        "{line}"
    );

    let neither = Observation::default();
    let line = verdict(&neither, 2);
    assert!(
        line.contains("at all: NO") && line.contains("visible: NO"),
        "{line}"
    );
}

#[test]
fn a_ctrl_v_keydown_is_not_taken_for_the_chord() {
    // Ctrl+V (no Shift) must not count as the Ctrl+Shift+V keydown.
    let observation = Observation {
        chord: case(vec![key("keydown", false)], "a", "a"),
        ..Default::default()
    };
    assert!(verdict(&observation, 2).contains("keydown is visible: NO"));
}

#[test]
fn a_paste_before_its_keydown_is_reported_as_after() {
    let observation = Observation {
        ctrl_v: case(
            vec![
                paste(&["text/html", "text/plain"], 5, 5),
                key("keydown", false),
            ],
            "a",
            "a",
        ),
        ..Default::default()
    };
    assert!(text(&observation).contains("the keydown came AFTER the paste event"));
}

#[test]
fn a_constructed_event_that_reaches_a_listener_is_verdict_three_yes() {
    let observation = Observation {
        constructed: Constructed {
            ok: true,
            data_transfer_types: Some(vec!["text/html".into(), "text/plain".into()]),
            event_has_clipboard_data: Some(true),
            event_types: Some(vec!["text/html".into(), "text/plain".into()]),
            not_cancelled: Some(true),
            doc_changed: Some(false),
            reached: vec![paste(&["text/html", "text/plain"], 30, 13)],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        verdict(&observation, 3).contains("reaches a listener: YES"),
        "{}",
        verdict(&observation, 3)
    );
    let all = text(&observation);
    assert!(
        all.contains("1 paste event(s) reached the listener"),
        "{all}"
    );
    assert!(all.contains("what the listener read from it"), "{all}");
}

#[test]
fn a_constructed_event_that_fails_or_does_not_arrive_is_verdict_three_no() {
    let failed = Observation {
        constructed: Constructed {
            error: Some("TypeError: illegal constructor".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        text(&failed)
            .contains("Q3 constructing a ClipboardEvent failed: TypeError: illegal constructor")
    );
    assert!(verdict(&failed, 3).contains("reaches a listener: NO"));

    let lost = Observation {
        constructed: Constructed {
            ok: true,
            event_has_clipboard_data: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(verdict(&lost, 3).contains("reaches a listener: NO"));
    // A constructed event with no text/html is not a "yes" either.
    let plain_only = Observation {
        constructed: Constructed {
            ok: true,
            reached: vec![paste(&["text/plain"], 0, 5)],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(verdict(&plain_only, 3).contains("reaches a listener: NO"));
}

#[test]
fn verdict_three_says_when_it_was_not_needed() {
    let observation = Observation {
        ctrl_v: both_flavours_case(),
        constructed: Constructed {
            ok: true,
            reached: vec![paste(&["text/html"], 5, 0)],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(verdict(&observation, 3).ends_with("(not needed: verdict 1 holds)"));
}

#[test]
fn a_helper_that_never_became_ready_is_the_first_line() {
    let observation = Observation {
        helper: Helper {
            ready: false,
            note: "the helper exited with exit status: 1; stderr: ModuleNotFoundError: gi".into(),
        },
        ..Default::default()
    };
    let first = answers(&observation).remove(0);
    assert!(
        first.contains("DID NOT become ready") && first.contains("ModuleNotFoundError: gi"),
        "{first}"
    );
}

// ---- the page's field names and Rust's agree --------------------------------

#[test]
fn the_json_the_page_produces_is_read_field_for_field() {
    let taken: Taken = serde_json::from_str(
        r#"{"events":[
            {"kind":"keydown","at":3,"target":"cm-content","trusted":true,"defaultPrevented":false,
             "key":"v","code":"KeyV","ctrl":true,"shift":false,"meta":false},
            {"kind":"paste","at":4,"target":"cm-content","trusted":true,"defaultPrevented":false,
             "hasClipboardData":true,"types":["text/html","text/plain"],
             "htmlLength":63,"htmlHead":"<h1>Probe","plainLength":40,"plainHead":"Probe heading"},
            {"kind":"beforeinput","at":5,"target":"cm-content","trusted":true,"defaultPrevented":false,
             "inputType":"insertFromPaste","data":null}],
           "doc":"start"}"#,
    )
    .unwrap();
    assert_eq!(taken.events.len(), 3);
    let event = &taken.events[1];
    assert_eq!(event.html_length, Some(63));
    assert_eq!(event.plain_head.as_deref(), Some("Probe heading"));
    assert_eq!(event.types.as_ref().unwrap(), &["text/html", "text/plain"]);
    assert_eq!(taken.events[0].code.as_deref(), Some("KeyV"));
    assert_eq!(
        taken.events[2].input_type.as_deref(),
        Some("insertFromPaste")
    );
    assert_eq!(taken.doc.as_deref(), Some("start"));

    let constructed: Constructed = serde_json::from_str(
        r#"{"ok":true,"dataTransferTypes":["text/html","text/plain"],"eventHasClipboardData":true,
            "eventTypes":["text/html"],"notCancelled":true,"docChanged":false,
            "reached":[{"kind":"paste","at":1,"trusted":false,"defaultPrevented":false,
                        "hasClipboardData":true,"types":["text/html"],"htmlLength":9,"plainLength":0}]}"#,
    )
    .unwrap();
    assert!(constructed.ok && constructed.not_cancelled == Some(true));
    assert_eq!(constructed.reached[0].html_length, Some(9));
    assert_eq!(constructed.doc_changed, Some(false));
}

/// Every field name Rust reads must be one the script writes, so a rename in one
/// place fails here and not silently in CI.
#[test]
fn every_field_the_report_reads_is_written_by_the_page_script() {
    let script = include_str!("paste_probe.js");
    for name in [
        "kind",
        "at",
        "target",
        "trusted",
        "defaultPrevented",
        "key",
        "code",
        "ctrl",
        "shift",
        "meta",
        "inputType",
        "data",
        "hasClipboardData",
        "types",
        "htmlLength",
        "htmlHead",
        "plainLength",
        "plainHead",
        "events",
        "doc",
        "ok",
        "error",
        "dataTransferTypes",
        "eventHasClipboardData",
        "eventTypes",
        "notCancelled",
        "docChanged",
        "reached",
    ] {
        assert!(
            script.contains(&format!("{name}:"))
                || script.contains(&format!(".{name} ="))
                || script.contains(&format!("{{ {name}"))
                || script.contains(&format!("{name},")),
            "paste_probe.js never writes {name:?}"
        );
    }
}
