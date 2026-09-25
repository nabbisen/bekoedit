// Unit tests for the guards and limits. The fixture corpus, which runs the real
// `mdka`, is in `tests/corpus.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::*;

const ANY_LIMITS: Limits = Limits {
    max_bytes: 100,
    budget: Duration::from_secs(5),
};

fn echo(html: String) -> String {
    html
}

fn convert_echo(html: &str, plain: &str, le: LineEnding) -> Outcome {
    convert_using(html, plain, le, DATA_IMAGE_MARKER, ANY_LIMITS, echo)
}

// ---- the size limit ------------------------------------------------------

#[test]
fn the_limit_is_one_mebibyte_and_the_budget_two_seconds() {
    assert_eq!(MAX_HTML_BYTES, 1_048_576);
    assert_eq!(CONVERSION_BUDGET, Duration::from_secs(2));
}

#[test]
fn html_at_the_limit_converts_and_one_byte_over_falls_back_without_calling_the_engine() {
    let called = Arc::new(AtomicBool::new(false));
    let seen = called.clone();
    let engine = move |html: String| {
        seen.store(true, Ordering::SeqCst);
        html
    };
    let at = "x".repeat(ANY_LIMITS.max_bytes);
    let ok = convert_using(&at, "p", LineEnding::Lf, "m", ANY_LIMITS, engine.clone());
    assert!(matches!(ok, Outcome::Converted { .. }), "{ok:?}");
    assert!(called.load(Ordering::SeqCst));

    called.store(false, Ordering::SeqCst);
    let over = "x".repeat(ANY_LIMITS.max_bytes + 1);
    let out = convert_using(&over, "p", LineEnding::Lf, "m", ANY_LIMITS, engine);
    assert_eq!(out, Outcome::Fallback(FallbackReason::TooLarge));
    assert!(
        !called.load(Ordering::SeqCst),
        "mdka must not run over the limit"
    );
}

#[test]
fn the_limit_is_measured_in_bytes_not_characters() {
    // 34 three-byte characters are 102 bytes.
    let html = "あ".repeat(34);
    assert_eq!(
        convert_echo(&html, "p", LineEnding::Lf),
        Outcome::Fallback(FallbackReason::TooLarge)
    );
}

// ---- a panic --------------------------------------------------------------

#[test]
fn a_panic_in_the_engine_is_caught_and_falls_back_as_failed() {
    let out = convert_using(
        "<p>x</p>",
        "x",
        LineEnding::Lf,
        "m",
        ANY_LIMITS,
        |_| -> String { panic!("boom") },
    );
    assert_eq!(out, Outcome::Fallback(FallbackReason::Failed));
}

#[test]
fn a_panic_carries_its_message_in_the_error() {
    let error = run_guarded("x", ANY_LIMITS, |_| -> String { panic!("boom {}", 7) }).unwrap_err();
    assert!(
        matches!(&error, ConvertError::Panicked(m) if m == "boom 7"),
        "{error}"
    );
    assert!(error.to_string().contains("boom 7"));
}

// ---- the time budget -----------------------------------------------------

#[test]
fn a_stalled_engine_times_out_promptly_and_its_late_result_is_dropped() {
    let finished = Arc::new(AtomicBool::new(false));
    let signal = finished.clone();
    let limits = Limits {
        max_bytes: 100,
        budget: Duration::from_millis(50),
    };
    let started = Instant::now();
    let out = convert_using("<p>x</p>", "x", LineEnding::Lf, "m", limits, move |html| {
        std::thread::sleep(Duration::from_millis(400));
        signal.store(true, Ordering::SeqCst);
        html
    });
    let waited = started.elapsed();
    assert_eq!(out, Outcome::Fallback(FallbackReason::TimedOut));
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
    assert!(
        !finished.load(Ordering::SeqCst),
        "the caller must not have waited for the stalled worker"
    );
    // The worker runs on and ends by itself; its late send finds no receiver.
    let deadline = Instant::now() + Duration::from_secs(3);
    while !finished.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(finished.load(Ordering::SeqCst), "the worker never finished");
}

#[test]
fn an_engine_inside_the_budget_is_not_timed_out() {
    let limits = Limits {
        max_bytes: 100,
        budget: Duration::from_secs(5),
    };
    let out = convert_using("hello", "hello", LineEnding::Lf, "m", limits, |html| {
        std::thread::sleep(Duration::from_millis(20));
        html
    });
    assert!(matches!(out, Outcome::Converted { .. }), "{out:?}");
}

// ---- empty ---------------------------------------------------------------

#[test]
fn blank_output_is_empty_whatever_the_plain_flavour_is() {
    for blank in ["", "   ", "\n\n", " \r\n \t"] {
        assert_eq!(
            convert_echo(blank, "text", LineEnding::Lf),
            Outcome::Empty,
            "{blank:?}"
        );
        assert_eq!(
            convert_echo(blank, "", LineEnding::Lf),
            Outcome::Empty,
            "{blank:?}"
        );
    }
    assert!(matches!(
        convert_echo("a", "", LineEnding::Lf),
        Outcome::Converted { .. }
    ));
}

// ---- line endings ---------------------------------------------------------

#[test]
fn every_line_break_becomes_the_target_ending() {
    let mixed = "a\r\nb\nc\rd\r\n\r\ne\n\nf  \ng";
    assert_eq!(
        normalize_line_endings(mixed, LineEnding::Lf),
        "a\nb\nc\nd\n\ne\n\nf  \ng"
    );
    assert_eq!(
        normalize_line_endings(mixed, LineEnding::Crlf),
        "a\r\nb\r\nc\r\nd\r\n\r\ne\r\n\r\nf  \r\ng"
    );
    // No stray CR or LF survives in the wrong shape.
    let crlf = normalize_line_endings(mixed, LineEnding::Crlf);
    assert_eq!(crlf.matches('\n').count(), crlf.matches("\r\n").count());
    assert!(!normalize_line_endings(mixed, LineEnding::Lf).contains('\r'));
}

#[test]
fn convert_applies_the_target_ending_to_the_result() {
    let Outcome::Converted { markdown, .. } = convert_echo("a\nb\r\nc", "p", LineEnding::Crlf)
    else {
        panic!("expected a conversion");
    };
    assert_eq!(markdown, "a\r\nb\r\nc");
}

// ---- html_had_table -------------------------------------------------------

#[test]
fn html_had_table_is_ascii_case_insensitive() {
    for html in [
        "<table>",
        "<TABLE>",
        "<TaBlE class=x>",
        "x<Table",
        "<p>a</p><TABLE",
    ] {
        assert!(contains_table(html), "{html}");
    }
    for html in [
        "",
        "<tabl",
        "table",
        "&lt;table&gt;",
        "<tab le>",
        "< table>",
        "<p>a</p>",
    ] {
        assert!(!contains_table(html), "{html}");
    }
    let Outcome::Converted { html_had_table, .. } = convert_echo("<TABLE>", "p", LineEnding::Lf)
    else {
        panic!("expected a conversion");
    };
    assert!(html_had_table);
}

// ---- the data: guard ------------------------------------------------------

fn guard(markdown: &str) -> String {
    guard::strip_data_destinations(markdown, "(M)")
}

#[test]
fn a_data_image_becomes_its_alt_text() {
    assert_eq!(guard("![pix](data:image/png;base64,AAAA)"), "pix");
    assert_eq!(guard("a ![pix](data:image/png;base64,AAAA) b"), "a pix b");
    assert_eq!(
        guard("![pix](data:image/png;base64,AAAA \"a title\")"),
        "pix"
    );
    // The shapes `mdka` writes for a data URI with spaces, quotes and
    // parentheses (fixtures 23 and 24): `<` and `>` escaped inside `<...>`, and
    // a single-quoted title.
    assert_eq!(
        guard(
            "x ![vector](<data:image/svg+xml;utf8,\\<svg xmlns='http://www.w3.org/2000/svg'\\>\\</svg\\>>) y"
        ),
        "x vector y"
    );
    assert_eq!(
        guard(
            "![paren (x) \\[y\\]](data:image/svg+xml,%3Csvg%20fill%3D%22url(%23a)%22%2F%3E 'a \"quoted\" title')"
        ),
        "paren (x) \\[y\\]"
    );
}

#[test]
fn a_data_image_with_no_alt_becomes_the_marker() {
    assert_eq!(guard("![](data:image/png;base64,AAAA)"), "(M)");
    assert_eq!(guard("x ![ ](data:image/gif;base64,R0lG) y"), "x (M) y");
    assert_eq!(
        guard("![](data:image/png;base64,A) and ![b](data:image/png;base64,B)"),
        "(M) and b"
    );
}

#[test]
fn the_scheme_is_matched_without_regard_to_case() {
    assert_eq!(guard("![a](DATA:image/png;base64,AA)"), "a");
    assert_eq!(guard("![a](Data:image/png;base64,AA)"), "a");
}

#[test]
fn an_angle_bracketed_destination_is_read_too() {
    assert_eq!(guard("![a](<data:image/png;base64,AA>)"), "a");
    assert_eq!(guard("![](<data:image/png;base64,AA> \"t\")"), "(M)");
}

#[test]
fn a_data_link_becomes_its_text() {
    assert_eq!(guard("[click](data:text/html;base64,PGh0bWw+)"), "click");
    assert_eq!(
        guard("see [click **me**](data:text/html,x) now"),
        "see click **me** now"
    );
}

#[test]
fn an_image_nested_in_a_link_is_replaced_and_the_link_kept() {
    assert_eq!(
        guard("[![pix](data:image/png;base64,AA)](https://example.com/x)"),
        "[pix](https://example.com/x)"
    );
    assert_eq!(
        guard("[![](data:image/png;base64,AA)](https://example.com/x)"),
        "[(M)](https://example.com/x)"
    );
}

#[test]
fn ordinary_links_and_images_are_untouched() {
    for md in [
        "[a](https://example.com/x)",
        "![a](https://example.com/x.png)",
        "![a](relative/path.png \"title\")",
        "[a](<https://example.com/a b>)",
        "[a]",
        "[a] (data:x)",
        "![a] (data:x)",
        "plain text with data:image/png;base64,AAAA in it",
        "[a](https://example.com/(x))",
        "日本語 ![画像](https://example.com/x.png) 日本語",
    ] {
        assert_eq!(guard(md), md);
    }
}

#[test]
fn code_is_never_touched() {
    for md in [
        "`![a](data:image/png;base64,AA)`",
        "x `![a](data:image/png;base64,AA)` y",
        "``a ` ![a](data:x) ` b``",
        "```\n![a](data:image/png;base64,AA)\n```\n",
        "~~~rust\n[a](data:x)\n~~~\n",
        "> ```\n> ![a](data:image/png;base64,AA)\n> ```\n",
        "- ```\n  ![a](data:image/png;base64,AA)\n  ```\n",
        "1. ```\n   [x](data:y)\n   ```\n",
        "````\n```\n![a](data:x)\n```\n````\n",
    ] {
        assert_eq!(guard(md), md, "{md:?}");
    }
}

#[test]
fn text_after_a_closed_fence_is_guarded_again() {
    assert_eq!(
        guard("```\n![a](data:x)\n```\n![b](data:y)\n"),
        "```\n![a](data:x)\n```\nb\n"
    );
    // An unclosed backtick is literal, not a code span.
    assert_eq!(guard("a ` ![b](data:y)"), "a ` b");
}

#[test]
fn an_escaped_bracket_is_not_a_link_but_the_construct_after_it_still_is() {
    assert_eq!(guard("\\[a](data:x)"), "\\[a](data:x)");
    assert_eq!(guard("\\![a](data:x)"), "\\!a");
}

#[test]
fn an_alt_with_brackets_and_parentheses_is_matched_whole() {
    assert_eq!(
        guard("![a \\[1\\] (b)](data:image/png;base64,AA)"),
        "a \\[1\\] (b)"
    );
    assert_eq!(
        guard("![a [nested] b](data:image/png;base64,AA)"),
        "a [nested] b"
    );
}

#[test]
fn lines_are_kept_line_for_line() {
    let md = "# t\n\n![a](data:x)\n\nlast";
    assert_eq!(guard(md), "# t\n\na\n\nlast");
    assert_eq!(guard(""), "");
    assert_eq!(guard("\n\n"), "\n\n");
}

#[test]
fn a_malformed_construct_is_left_alone_and_never_panics() {
    for md in [
        "![a](data:x",
        "![a](",
        "![a",
        "![",
        "[",
        "]",
        "![a](<data:x",
        "![a](data:x \"t",
        "![a](data:x \"t\" junk)",
        "[[[",
        "]]]",
        "![[[](data:x)",
        "\\",
        "`",
        "``",
        "![a](data:x)\\",
    ] {
        let _ = guard(md);
    }
}

// ---- a panic must be catchable -------------------------------------------

/// `catch_unwind` cannot catch anything under `panic = "abort"`, and the whole
/// process would go down with `mdka`. Nothing sets it; this keeps it so.
#[test]
fn no_profile_in_the_workspace_aborts_on_panic() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for manifest in [root.join("Cargo.toml"), root.join("../../Cargo.toml")] {
        let text = std::fs::read_to_string(&manifest).unwrap();
        let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            !squeezed.contains("panic=\"abort\""),
            "{} sets panic = \"abort\", which defeats the conversion's panic guard",
            manifest.display()
        );
    }
}
