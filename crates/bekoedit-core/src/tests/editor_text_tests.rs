//! Task 027 §6.2: Text Mode's editor text is reconciled with the canonical
//! text, so the file's own line endings survive. A matrix of texts and edits,
//! checked against an independent oracle, not against the implementation.

use crate::DocumentSession;
use crate::editor_text::{editor_form, editor_position, reconcile};

const TEXTS: &[(&str, &str)] = &[
    ("lf", "# T\nline two\nlast\n"),
    ("crlf", "# T\r\nline two\r\nlast\r\n"),
    ("mixed", "# T\r\nline two\nlast\r\nend"),
    ("lone cr", "a\rb\rc"),
    ("crlf, no trailing newline", "one\r\ntwo"),
    ("empty", ""),
    ("single line", "just one line"),
    ("multibyte crlf", "héllo wörld\r\n日本語\r\n🙂 end\r\n"),
    ("multibyte mixed", "日本\r\n語\n🙂 tail"),
];

/// Independent oracle: the canonical byte offset of an editor-form byte
/// offset, by walking the canonical text one character at a time.
fn canonical_offset(canonical: &str, editor_offset: usize) -> usize {
    let (mut editor, mut at) = (0, 0);
    while editor < editor_offset {
        let rest = &canonical[at..];
        if rest.starts_with("\r\n") {
            at += 2;
            editor += 1;
        } else {
            let ch = rest.chars().next().unwrap();
            at += ch.len_utf8();
            editor += if ch == '\r' { 1 } else { ch.len_utf8() };
        }
    }
    at
}

fn middle(text: &str) -> usize {
    text.char_indices()
        .nth(text.chars().count() / 2)
        .map_or(text.len(), |(index, _)| index)
}

/// `(name, editor start, editor end, replacement)` over an editor form.
fn edits(editor: &str) -> Vec<(&'static str, usize, usize, &'static str)> {
    let mid = middle(editor);
    let mut out = vec![
        ("insert at start", 0, 0, "X"),
        ("insert in the middle", mid, mid, "X"),
        ("insert at the end", editor.len(), editor.len(), "X"),
        ("insert a newline", mid, mid, "\n"),
    ];
    if let Some(first) = editor.find('\n') {
        out.push((
            "delete across a break",
            first.saturating_sub(1).min(first),
            first + 1,
            "",
        ));
        out.push(("delete a whole line", 0, first + 1, ""));
    }
    out
}

fn apply(editor: &str, start: usize, end: usize, replacement: &str) -> String {
    format!("{}{replacement}{}", &editor[..start], &editor[end..])
}

#[test]
fn every_edit_keeps_the_editor_view_and_every_byte_outside_the_edit() {
    for (name, canonical) in TEXTS {
        let editor = editor_form(canonical);
        for (edit, start, end, replacement) in edits(&editor) {
            // Keep the range on char boundaries (the deletes above use bytes).
            if !editor.is_char_boundary(start) || !editor.is_char_boundary(end) {
                continue;
            }
            let incoming = apply(&editor, start, end, replacement);
            let result = reconcile(canonical, &incoming);
            let what = format!("{name} / {edit}: {canonical:?} -> {result:?}");
            assert_eq!(editor_form(&result), incoming, "the editor's view: {what}");
            let (a, b) = (
                canonical_offset(canonical, start),
                canonical_offset(canonical, end),
            );
            assert!(
                result.starts_with(&canonical[..a]),
                "bytes before the edit: {what}"
            );
            assert!(
                result.ends_with(&canonical[b..]),
                "bytes after the edit: {what}"
            );
        }
    }
}

#[test]
fn a_text_that_is_just_its_own_editor_form_is_a_no_op() {
    for (name, canonical) in TEXTS {
        assert_eq!(
            &reconcile(canonical, &editor_form(canonical)),
            canonical,
            "{name}"
        );
        let mut session = DocumentSession::from_text(1, "/x.md".into(), (*canonical).into());
        let revision = session.revision;
        assert_eq!(
            session.apply_editor_text(revision, &editor_form(canonical)),
            Ok(false),
            "{name}"
        );
        assert_eq!(
            (session.revision, session.dirty),
            (revision, false),
            "{name}"
        );
        assert_eq!(&session.canonical_text, canonical, "{name}");
        assert!(session.matches_editor_text(&editor_form(canonical)));
    }
}

#[test]
fn a_uniform_file_keeps_its_ending_for_every_edit_including_two_places() {
    for (name, canonical, ending) in [
        ("lf", "x one\nx two\nx three\n", "\n"),
        ("crlf", "x one\r\nx two\r\nx three\r\n", "\r\n"),
        ("crlf, no trailing newline", "x one\r\nx two", "\r\n"),
    ] {
        let replaced = editor_form(canonical).replace('x', "YY\nY");
        let result = reconcile(canonical, &replaced);
        assert_eq!(
            result,
            replaced.replace('\n', ending),
            "{name}: replace-all"
        );
        assert!(!result.contains('\r') || ending == "\r\n", "{name}");
        assert_eq!(
            result.matches('\n').count(),
            result.matches(ending).count(),
            "{name}"
        );
    }
}

#[test]
fn a_mixed_file_changes_only_the_line_it_touches() {
    let canonical = "# T\r\nline two\nlast\r\nend";
    // Insert on line 2, an LF line: the new break is that line's own ending.
    let result = reconcile(canonical, "# T\nline\nX two\nlast\nend");
    assert_eq!(result, "# T\r\nline\nX two\nlast\r\nend");
    // Insert a break in the CRLF first line.
    assert_eq!(
        reconcile(canonical, "# \nT\nline two\nlast\nend"),
        "# \r\nT\r\nline two\nlast\r\nend"
    );
    // Typing on the last, unterminated line takes the break before it.
    assert_eq!(
        reconcile(canonical, "# T\nline two\nlast\nend\nmore"),
        "# T\r\nline two\nlast\r\nend\r\nmore"
    );
    // Deleting the LF line whole leaves its neighbours' endings alone.
    assert_eq!(reconcile(canonical, "# T\nlast\nend"), "# T\r\nlast\r\nend");
}

#[test]
fn a_lone_cr_file_keeps_its_carriage_returns() {
    assert_eq!(reconcile("a\rb\rc", "a\nbX\nc"), "a\rbX\rc");
    assert_eq!(reconcile("a\rb\rc", "a\nb\nc\nd"), "a\rb\rc\rd");
}

#[test]
fn a_file_with_no_breaks_gets_lf_for_a_new_line() {
    assert_eq!(reconcile("one", "one\ntwo"), "one\ntwo");
    assert_eq!(reconcile("", "\n"), "\n");
}

#[test]
fn a_boundary_never_splits_a_crlf() {
    // Deleting exactly the break between two lines, in a mixed file.
    let canonical = "a\r\nb\nc";
    assert_eq!(reconcile(canonical, "ab\nc"), "ab\nc");
    assert_eq!(reconcile(canonical, "a\nbc"), "a\r\nbc");
}

#[test]
fn a_stale_revision_is_still_rejected() {
    let mut session = DocumentSession::from_text(1, "/x.md".into(), "a\r\n".into());
    assert!(session.apply_editor_text(99, "a\nb\n").is_err());
    assert_eq!(session.canonical_text, "a\r\n");
}

#[test]
fn an_editor_position_counts_editor_form_utf16_units_not_canonical_bytes() {
    // LF: bytes and units agree.
    assert_eq!(editor_position("# a\n## b\n", 4), 4);
    // CRLF: each preceding break is one unit in the editor, two bytes here.
    let crlf = "# a\r\n## b\r\n";
    assert_eq!(editor_position(crlf, crlf.find("## b").unwrap()), 4);
    // Multibyte: a byte is not a unit. "日本" is 6 bytes, 2 units; the emoji is 4 bytes, 2 units.
    let jp = "日本\r\n🙂 x";
    assert_eq!(editor_position(jp, jp.find('x').unwrap()), 2 + 1 + 2 + 1);
    // Between one break's CR and LF: before the break. Past the end: the end.
    assert_eq!(editor_position("ab\r\ncd", 3), 2);
    assert_eq!(editor_position("ab\r\ncd", 99), 5);
    // Not on a character boundary: falls back to the boundary before it.
    assert_eq!(editor_position("日", 1), 0);
}
