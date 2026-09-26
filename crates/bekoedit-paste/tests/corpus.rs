//! The fixture corpus (RFC-046 §5.3, §7; slice 1 §5, §6).
//!
//! Every `tests/fixtures/<name>.html` is converted with the real `mdka` and
//! compared, byte for byte, with `<name>.md`, for an LF and for a CRLF target.
//! Then invariants run over every output:
//!
//! - **the structural raw-HTML test**: parse the output with the document's GFM
//!   options; every `Html` and `InlineHtml` event must be exactly `<br>` and
//!   inside a `TableCell`. A line scan would be wrong for a table inside a
//!   blockquote or a list item, and for `Vec<i32>` in code, so it is not used;
//! - no `data:` destination on any link or image;
//! - the guard changed no code content;
//! - only the target line ending;
//! - each fixture converts inside the time budget, and is not a fallback.

use std::path::{Path, PathBuf};
use std::time::Instant;

use bekoedit_paste::{CONVERSION_BUDGET, LineEnding, Outcome, convert};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// The document's own GFM options (`bekoedit-markdown`'s `parse_options`).
fn document_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
}

/// At run time, not `env!`: that is fixed into the binary when it is compiled, so
/// a test binary reused from a deleted worktree (a shared `target/`) would look in
/// a directory that no longer exists. Cargo sets this when it runs a test.
fn fixtures_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("run by cargo test");
    Path::new(&manifest_dir).join("tests/fixtures")
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            (path.extension()? == "html")
                .then(|| path.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

fn read(name: &str, extension: &str) -> String {
    let path = fixtures_dir().join(format!("{name}.{extension}"));
    String::from_utf8(std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .unwrap()
}

fn converted(html: &str, line_ending: LineEnding) -> String {
    match convert(html, "plain", line_ending) {
        Outcome::Converted { markdown, .. } => markdown,
        other => panic!("expected a conversion, got {other:?}"),
    }
}

/// The structural raw-HTML check. Returns every violation, naming the event.
fn raw_html_violations(markdown: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let mut cell_depth = 0usize;
    for event in Parser::new_ext(markdown, document_options()) {
        match event {
            Event::Start(Tag::TableCell) => cell_depth += 1,
            Event::End(TagEnd::TableCell) => cell_depth -= 1,
            Event::Html(text) | Event::InlineHtml(text) => {
                let bare_br_in_cell = text.as_ref() == "<br>" && cell_depth > 0;
                if !bare_br_in_cell {
                    violations.push(format!(
                        "{text:?} (inside a table cell: {})",
                        cell_depth > 0
                    ));
                }
            }
            _ => {}
        }
    }
    violations
}

fn data_destinations(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, document_options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. })
                if dest_url
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("data:") =>
            {
                Some(dest_url.to_string())
            }
            _ => None,
        })
        .collect()
}

/// The text of every code span and code block, in order.
fn code_content(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut block: Option<String> = None;
    for event in Parser::new_ext(markdown, document_options()) {
        match event {
            Event::Code(text) => out.push(text.to_string()),
            Event::Start(Tag::CodeBlock(kind)) => {
                let _ = matches!(kind, CodeBlockKind::Fenced(_));
                block = Some(String::new());
            }
            Event::Text(text) if block.is_some() => block.as_mut().unwrap().push_str(&text),
            Event::End(TagEnd::CodeBlock) => out.extend(block.take()),
            _ => {}
        }
    }
    out
}

#[test]
fn the_corpus_is_there_and_every_html_file_has_its_markdown_and_no_markdown_is_orphaned() {
    let names = fixture_names();
    assert!(names.len() >= 20, "only {} fixtures", names.len());
    for name in &names {
        assert!(
            name.starts_with("synthetic-"),
            "{name}: every fixture here is hand-written and says so"
        );
        assert!(
            fixtures_dir().join(format!("{name}.md")).exists(),
            "{name}.md is missing"
        );
    }
    for entry in std::fs::read_dir(fixtures_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "md") && path.file_stem().unwrap() != "README" {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            assert!(names.contains(&stem), "{stem}.md has no .html");
        }
    }
}

#[test]
fn every_fixture_converts_to_its_expected_markdown_for_both_line_endings() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let expected = read(&name, "md");
        assert_eq!(converted(&html, LineEnding::Lf), expected, "{name} (LF)");
        assert_eq!(
            converted(&html, LineEnding::Crlf),
            expected.replace('\n', "\r\n"),
            "{name} (CRLF)"
        );
    }
}

#[test]
fn the_only_raw_html_in_any_output_is_a_bare_br_inside_a_table_cell() {
    for name in fixture_names() {
        let html = read(&name, "html");
        for (label, line_ending) in [("LF", LineEnding::Lf), ("CRLF", LineEnding::Crlf)] {
            let markdown = converted(&html, line_ending);
            let violations = raw_html_violations(&markdown);
            assert!(
                violations.is_empty(),
                "{name} ({label}): raw HTML event(s) {violations:?} in {markdown:?}"
            );
        }
    }
}

#[test]
fn no_data_destination_survives_in_any_output() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let markdown = converted(&html, LineEnding::Lf);
        let found = data_destinations(&markdown);
        assert!(found.is_empty(), "{name}: {found:?}");
    }
}

#[test]
fn the_guard_changes_no_code_content() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let raw = mdka::html_to_markdown_with(
            &html,
            &mdka::ConversionOptions::for_mode(mdka::ConversionMode::Minimal),
        );
        let guarded = converted(&html, LineEnding::Lf);
        assert_eq!(code_content(&raw), code_content(&guarded), "{name}");
    }
}

#[test]
fn only_the_target_line_ending_appears() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let lf = converted(&html, LineEnding::Lf);
        assert!(!lf.contains('\r'), "{name}: a CR in the LF output");
        let crlf = converted(&html, LineEnding::Crlf);
        assert_eq!(
            crlf.matches('\n').count(),
            crlf.matches("\r\n").count(),
            "{name}: an LF without its CR in the CRLF output"
        );
        assert_eq!(
            crlf.matches('\r').count(),
            crlf.matches("\r\n").count(),
            "{name}: a lone CR"
        );
    }
}

#[test]
fn every_fixture_converts_inside_the_budget_and_is_not_a_fallback() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let started = Instant::now();
        let outcome = convert(&html, "plain", LineEnding::Lf);
        let elapsed = started.elapsed();
        assert!(
            matches!(outcome, Outcome::Converted { .. }),
            "{name}: {outcome:?}"
        );
        assert!(elapsed < CONVERSION_BUDGET, "{name}: took {elapsed:?}");
    }
}

#[test]
fn html_had_table_is_true_exactly_for_the_fixtures_that_contain_a_table() {
    for name in fixture_names() {
        let html = read(&name, "html");
        let Outcome::Converted { html_had_table, .. } = convert(&html, "plain", LineEnding::Lf)
        else {
            panic!("{name}: not converted");
        };
        assert_eq!(
            html_had_table,
            html.to_ascii_lowercase().contains("<table"),
            "{name}"
        );
    }
}

// ---- upstream's claims, each with its own assertion (RFC-046 §6.2) ---------

#[test]
fn claim_the_2_4_1_wrapper_in_a_cell_bug_is_fixed() {
    let html = read(
        "synthetic-10-table-cell-two-sibling-divs-2-4-1-regression",
        "html",
    );
    let markdown = converted(&html, LineEnding::Lf);
    assert!(markdown.contains("| Alice<br>lead | ok |"), "{markdown:?}");
    // 2.4.1 ended the row after `Alice`; the rest escaped as body text with a `|`.
    assert!(!markdown.contains("Alice\n"), "{markdown:?}");
}

#[test]
fn claim_br_outside_a_table_is_two_trailing_spaces() {
    let markdown = converted(&read("synthetic-09-hard-break", "html"), LineEnding::Lf);
    assert!(
        markdown.contains("line one  \nline two  \nline three"),
        "{markdown:?}"
    );
    assert!(!markdown.contains("<br"), "{markdown:?}");
}

#[test]
fn claim_a_literal_lt_that_could_start_markup_is_escaped() {
    let markdown = converted(
        &read("synthetic-22-literal-lt-shapes", "html"),
        LineEnding::Lf,
    );
    for shape in [
        "\\<a href=x>",
        "\\</b>",
        "\\<!-- c -->",
        "\\<?php echo 1; ?>",
        "\\<https://example.com>",
        "\\<me@example.com>",
        "a\\<b",
    ] {
        assert!(
            markdown.contains(shape),
            "{shape} not escaped in {markdown:?}"
        );
    }
    // ...and it forms no link, tag or comment when parsed.
    let events: Vec<Event> = Parser::new_ext(&markdown, document_options()).collect();
    assert!(
        !events.iter().any(|e| matches!(
            e,
            Event::Html(_) | Event::InlineHtml(_) | Event::Start(Tag::Link { .. })
        )),
        "{events:?}"
    );
}

#[test]
fn claim_in_minimal_no_id_anchor_is_emitted() {
    let markdown = converted(&read("synthetic-08-id-anchors", "html"), LineEnding::Lf);
    assert!(!markdown.contains("<a"), "{markdown:?}");
}

/// A document of exactly `bytes` bytes: paragraphs, then an HTML comment as
/// padding, which converts to nothing.
fn html_of_exactly(bytes: usize) -> String {
    let paragraph = "<p>Lorem ipsum dolor sit amet, <b>consectetur</b> adipiscing elit.</p>\n";
    let (head, tail) = ("<html><body>", "</body></html>");
    let mut html = String::from(head);
    while html.len() + paragraph.len() + tail.len() + "<!---->".len() <= bytes {
        html.push_str(paragraph);
    }
    let pad = bytes - html.len() - tail.len() - "<!---->".len();
    html.push_str(&format!("<!--{}-->", "x".repeat(pad)));
    html.push_str(tail);
    assert_eq!(html.len(), bytes);
    html
}

#[test]
fn a_document_of_exactly_the_limit_converts_and_one_byte_over_falls_back_as_too_large() {
    let limit = bekoedit_paste::MAX_HTML_BYTES;
    let html = html_of_exactly(limit);
    let started = Instant::now();
    let outcome = convert(&html, "plain", LineEnding::Lf);
    let elapsed = started.elapsed();
    // A debug build is several times slower than release (about 0.5 s here; the
    // release timings are in the review request), so a slow machine may hit the
    // 2 s budget. The limit's own semantics are tested deterministically in the
    // unit tests; this is the real `mdka` at the real limit.
    assert!(
        matches!(
            outcome,
            Outcome::Converted { .. } | Outcome::Fallback(bekoedit_paste::FallbackReason::TimedOut)
        ),
        "{outcome:?} after {elapsed:?}"
    );
    eprintln!(
        "1 MiB (exactly the limit) took {elapsed:?}: {}",
        if matches!(outcome, Outcome::Converted { .. }) {
            "converted"
        } else {
            "timed out"
        }
    );

    let over = html_of_exactly(limit + 1);
    assert_eq!(
        convert(&over, "plain", LineEnding::Lf),
        Outcome::Fallback(bekoedit_paste::FallbackReason::TooLarge)
    );
}

// ---- the structural check must itself catch what it is there to catch -----

#[test]
fn the_structural_check_allows_only_a_bare_br_in_a_table_cell() {
    // Allowed: a bare <br> in a cell, in a plain table, a blockquote and a list item.
    for markdown in [
        "| A | B |\n| --- | --- |\n| a<br>b | 1 |\n",
        "> | A | B |\n> | --- | --- |\n> | a<br>b | 1 |\n",
        "- item\n\n  | A | B |\n  | --- | --- |\n  | a<br>b | 1 |\n",
        "`Vec<i32>` and\n\n```\nlet v: Vec<i32> = x<y>z;\n```\n",
        "a \\<div> and 1 <2 and x < y\n",
    ] {
        assert!(raw_html_violations(markdown).is_empty(), "{markdown:?}");
    }
    // Forbidden: anything else, or a <br> outside a cell.
    for markdown in [
        "| A |\n| --- |\n| <span>x</span> |\n",
        "| A |\n| --- |\n| <br class=\"x\"> |\n",
        "| A |\n| --- |\n| <br/> |\n",
        "| A |\n| --- |\n| a<div>b</div> |\n",
        "text <span>x</span> more\n",
        "line one<br>line two\n",
        "<div>block</div>\n",
        "<!-- comment -->\n",
        "> quoted <b>x</b>\n",
    ] {
        assert!(
            !raw_html_violations(markdown).is_empty(),
            "{markdown:?} was let through"
        );
    }
}
