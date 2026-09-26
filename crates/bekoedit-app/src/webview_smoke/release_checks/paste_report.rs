//! RFC-046 slice 2, part A: turns what the paste probe saw into answers.
//!
//! The probe (`paste_probe.rs`) sends real key chords and a constructed event to a
//! live WebView and records what the page saw. This module is the pure half: it
//! reads those recordings and writes one line per question the slice rests on. It
//! asserts nothing about product behaviour; a probe that observed and reported has
//! passed, whatever the answers are.

use serde::Deserialize;

/// One event the page recorded (`paste_probe.js`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct ProbeEvent {
    pub kind: String,
    pub at: u64,
    pub target: Option<String>,
    pub trusted: bool,
    pub default_prevented: bool,
    pub key: Option<String>,
    pub code: Option<String>,
    pub ctrl: Option<bool>,
    pub shift: Option<bool>,
    pub meta: Option<bool>,
    pub input_type: Option<String>,
    pub data: Option<String>,
    pub has_clipboard_data: Option<bool>,
    pub types: Option<Vec<String>>,
    pub html_length: Option<usize>,
    pub html_head: Option<String>,
    pub plain_length: Option<usize>,
    pub plain_head: Option<String>,
}

/// What `__pasteProbe.take()` returns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Taken {
    pub events: Vec<ProbeEvent>,
    pub doc: Option<String>,
}

/// What `__pasteProbe.construct()` returns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct Constructed {
    pub ok: bool,
    pub error: Option<String>,
    pub data_transfer_types: Option<Vec<String>>,
    pub event_has_clipboard_data: Option<bool>,
    pub event_types: Option<Vec<String>>,
    pub not_cancelled: Option<bool>,
    pub doc_changed: Option<bool>,
    pub reached: Vec<ProbeEvent>,
}

/// One key chord: the editor's text before, the events, and the text after.
#[derive(Debug, Clone, Default)]
pub(super) struct Case {
    pub events: Vec<ProbeEvent>,
    pub doc_before: Option<String>,
    pub doc_after: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Helper {
    /// The clipboard owner reported that it holds the selection.
    pub ready: bool,
    /// Why not, or what it said.
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Observation {
    pub helper: Helper,
    pub ctrl_v: Case,
    pub chord: Case,
    pub constructed: Constructed,
}

const HTML: &str = "text/html";
const PLAIN: &str = "text/plain";

fn yes(value: bool) -> &'static str {
    if value { "YES" } else { "NO" }
}

fn paste_event(case: &Case) -> Option<&ProbeEvent> {
    case.events.iter().find(|event| event.kind == "paste")
}

fn carries(event: &ProbeEvent, flavour: &str) -> bool {
    event
        .types
        .as_ref()
        .is_some_and(|types| types.iter().any(|t| t == flavour))
}

/// `text/html` and `text/plain` both listed, and both non-empty.
fn has_both(event: &ProbeEvent) -> bool {
    carries(event, HTML)
        && carries(event, PLAIN)
        && event.html_length.unwrap_or(0) > 0
        && event.plain_length.unwrap_or(0) > 0
}

/// A keydown for Ctrl+V, with or without Shift.
fn chord_keydown(case: &Case, shift: bool) -> Option<&ProbeEvent> {
    case.events.iter().find(|event| {
        event.kind == "keydown"
            && event.ctrl == Some(true)
            && event.shift == Some(shift)
            && event
                .key
                .as_deref()
                .is_some_and(|key| key.eq_ignore_ascii_case("v"))
    })
}

/// Whether the keydown was seen before the paste event, by the order recorded.
fn keydown_before_paste(case: &Case, shift: bool) -> Option<bool> {
    let keydown = case.events.iter().position(|e| {
        e.kind == "keydown"
            && e.ctrl == Some(true)
            && e.shift == Some(shift)
            && e.key
                .as_deref()
                .is_some_and(|k| k.eq_ignore_ascii_case("v"))
    })?;
    let paste = case.events.iter().position(|e| e.kind == "paste")?;
    Some(keydown < paste)
}

fn doc_delta(case: &Case) -> String {
    match (&case.doc_before, &case.doc_after) {
        (Some(before), Some(after)) if before == after => {
            "the editor text did not change".to_string()
        }
        (Some(before), Some(after)) => format!(
            "the editor text changed, {} to {} characters",
            before.chars().count(),
            after.chars().count()
        ),
        _ => "the editor text could not be read".to_string(),
    }
}

fn kinds(case: &Case) -> String {
    let list: Vec<String> = case
        .events
        .iter()
        .map(|event| match event.kind.as_str() {
            "keydown" | "keyup" => format!(
                "{} {}{}{}",
                event.kind,
                if event.ctrl == Some(true) {
                    "Ctrl+"
                } else {
                    ""
                },
                if event.shift == Some(true) {
                    "Shift+"
                } else {
                    ""
                },
                event.key.clone().unwrap_or_default()
            ),
            "beforeinput" | "input" => format!(
                "{} {}",
                event.kind,
                event.input_type.clone().unwrap_or_default()
            ),
            other => other.to_string(),
        })
        .collect();
    if list.is_empty() {
        "no events at all".to_string()
    } else {
        list.join(" > ")
    }
}

fn describe_paste(event: &ProbeEvent) -> String {
    format!(
        "clipboardData present: {}; types {:?}; text/html {} chars (starts {:?}); text/plain {} chars (starts {:?}); target {:?}; trusted: {}",
        yes(event.has_clipboard_data == Some(true)),
        event.types.clone().unwrap_or_default(),
        event.html_length.unwrap_or(0),
        event.html_head.clone().unwrap_or_default(),
        event.plain_length.unwrap_or(0),
        event.plain_head.clone().unwrap_or_default(),
        event.target.clone().unwrap_or_default(),
        yes(event.trusted),
    )
}

/// One line per question, then the verdict lines. Greppable: each verdict carries
/// a `YES` or `NO`.
pub(super) fn answers(observation: &Observation) -> Vec<String> {
    let mut lines = Vec::new();
    let helper = &observation.helper;
    lines.push(format!(
        "clipboard owner: {}{}",
        if helper.ready {
            "holds the selection with text/html and text/plain"
        } else {
            "DID NOT become ready"
        },
        if helper.note.is_empty() {
            String::new()
        } else {
            format!(" ({})", helper.note)
        }
    ));

    // 1. A real Ctrl+V.
    let ctrl_v = &observation.ctrl_v;
    let q1_event = paste_event(ctrl_v);
    lines.push(format!("Q1 real Ctrl+V, events seen: {}", kinds(ctrl_v)));
    lines.push(match q1_event {
        Some(event) => format!(
            "Q1 the paste event: {}; {}",
            describe_paste(event),
            doc_delta(ctrl_v)
        ),
        None => format!("Q1 no paste event reached the page; {}", doc_delta(ctrl_v)),
    });

    // 2. The plain-paste chord.
    let chord = &observation.chord;
    let q2_event = paste_event(chord);
    lines.push(format!(
        "Q2 real Ctrl+Shift+V, events seen: {}",
        kinds(chord)
    ));
    lines.push(match q2_event {
        Some(event) => format!(
            "Q2 the paste event: {}; {}",
            describe_paste(event),
            doc_delta(chord)
        ),
        None => format!("Q2 no paste event reached the page; {}", doc_delta(chord)),
    });
    for (case, shift, label) in [(ctrl_v, false, "Ctrl+V"), (chord, true, "Ctrl+Shift+V")] {
        if let Some(order) = keydown_before_paste(case, shift) {
            lines.push(format!(
                "Q-order {label}: the keydown came {} the paste event",
                if order { "BEFORE" } else { "AFTER" }
            ));
        }
    }

    // 3. A constructed event.
    let built = &observation.constructed;
    lines.push(match &built.error {
        Some(error) => format!("Q3 constructing a ClipboardEvent failed: {error}"),
        None => format!(
            "Q3 constructed ClipboardEvent: DataTransfer types {:?}; event.clipboardData present: {} with types {:?}; dispatched on the editor, not cancelled: {}; {} paste event(s) reached the listener; the editor text {}",
            built.data_transfer_types.clone().unwrap_or_default(),
            yes(built.event_has_clipboard_data == Some(true)),
            built.event_types.clone().unwrap_or_default(),
            yes(built.not_cancelled == Some(true)),
            built.reached.iter().filter(|e| e.kind == "paste").count(),
            if built.doc_changed == Some(true) { "changed" } else { "did not change" },
        ),
    });
    if let Some(event) = built.reached.iter().find(|e| e.kind == "paste") {
        lines.push(format!(
            "Q3 what the listener read from it: {}",
            describe_paste(event)
        ));
    }

    // The verdicts.
    let q1 = q1_event.is_some_and(has_both);
    let q3_reaches = built
        .reached
        .iter()
        .any(|e| e.kind == "paste" && carries(e, HTML));
    lines.push(format!(
        "VERDICT 1: a real Ctrl+V reaches a paste listener with BOTH flavours: {}",
        yes(q1)
    ));
    lines.push(format!(
        "VERDICT 2: Ctrl+Shift+V dispatches a paste event at all: {}; its keydown is visible: {}",
        yes(q2_event.is_some()),
        yes(chord_keydown(chord, true).is_some())
    ));
    lines.push(format!(
        "VERDICT 3: a constructed paste with text/html reaches a listener: {}{}",
        yes(q3_reaches),
        if q1 {
            " (not needed: verdict 1 holds)"
        } else {
            ""
        }
    ));
    lines
}
