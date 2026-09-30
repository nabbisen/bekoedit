use super::*;

#[test]
fn converted_without_a_table_raises_no_notice() {
    let (outcome, notice) = classify(Outcome::Converted {
        markdown: "# heading\n".into(),
        html_had_table: false,
    });
    assert_eq!(
        outcome,
        PasteOutcome::Converted {
            markdown: "# heading\n".into(),
            table_no_gfm_form: false
        }
    );
    assert_eq!(notice, None);
}

#[test]
fn converted_with_a_table_that_kept_its_gfm_form_raises_no_notice() {
    let (outcome, notice) = classify(Outcome::Converted {
        markdown: "| a | b |\n| - | - |\n| 1 | 2 |\n".into(),
        html_had_table: true,
    });
    assert_eq!(
        outcome,
        PasteOutcome::Converted {
            markdown: "| a | b |\n| - | - |\n| 1 | 2 |\n".into(),
            table_no_gfm_form: false,
        }
    );
    assert_eq!(notice, None);
}

#[test]
fn converted_with_a_table_that_lost_its_gfm_form_raises_the_table_notice() {
    let (outcome, notice) = classify(Outcome::Converted {
        markdown: "a\tb\n1\t2\n".into(),
        html_had_table: true,
    });
    assert_eq!(
        outcome,
        PasteOutcome::Converted {
            markdown: "a\tb\n1\t2\n".into(),
            table_no_gfm_form: true
        }
    );
    assert_eq!(notice, Some("paste.table_no_form"));
}

#[test]
fn fallback_reasons_map_to_their_wire_variant_and_notice() {
    let cases = [
        (
            FallbackReason::TooLarge,
            PasteFallbackReason::TooLarge,
            "paste.too_large",
        ),
        (
            FallbackReason::Failed,
            PasteFallbackReason::Failed,
            "paste.failed",
        ),
        (
            FallbackReason::TimedOut,
            PasteFallbackReason::TimedOut,
            "paste.timed_out",
        ),
    ];
    for (reason, wire, key) in cases {
        let (outcome, notice) = classify(Outcome::Fallback(reason));
        assert_eq!(outcome, PasteOutcome::Fallback { reason: wire });
        assert_eq!(notice, Some(key));
    }
}

// ---- RFC-046 §3.2/§3.6: the real converter is called with `LineEnding::Lf`
// and the localised marker -- no WebView needed, `bekoedit_paste` is a pure
// converter. ----

#[test]
fn convert_always_uses_lf_line_endings() {
    let markdown = match convert("<h1>A</h1><p>B</p><p>C</p>", "image") {
        Outcome::Converted { markdown, .. } => markdown,
        other => panic!("expected a conversion, got {other:?}"),
    };
    assert!(!markdown.contains("\r\n"), "{markdown:?}");
    assert!(markdown.contains('\n'), "{markdown:?}");
}

#[test]
fn convert_passes_the_given_marker_through_for_an_image_with_no_alt_text() {
    let markdown = match convert(r#"<p><img src="data:image/png;base64,AAAA"></p>"#, "がぞう") {
        Outcome::Converted { markdown, .. } => markdown,
        other => panic!("expected a conversion, got {other:?}"),
    };
    assert!(markdown.contains("がぞう"), "{markdown:?}");
}

#[test]
fn empty_raises_no_notice() {
    let (outcome, notice) = classify(Outcome::Empty);
    assert_eq!(outcome, PasteOutcome::Empty);
    assert_eq!(notice, None);
}
