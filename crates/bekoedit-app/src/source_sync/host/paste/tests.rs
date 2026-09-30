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

// ---- RFC-046 §3.3/§3.4 (review, 2026-09-30): exactly one notice per paste,
// resolved once the page says what happened to the reply. `resolve_applied`/
// `resolve_discarded` are pure over the shared pending-notice store, so every
// case is unit tested without a live `Signal`. Each test claims its own
// token: the store is process-global and tests run in parallel.

fn fresh_token() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1_000_000);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[test]
fn a_fallback_reply_that_is_then_discarded_yields_only_the_discard_notice() {
    let token = fresh_token();
    set_pending_notice(token, "paste.failed");
    assert_eq!(
        resolve_discarded(token),
        PendingResolution::Notify("paste.discarded", ToastKind::Warning)
    );
    // The dropped reply notice ("pasted as plain text: could not convert")
    // cannot surface later either -- nothing was actually pasted.
    assert_eq!(resolve_applied(token), PendingResolution::Nothing);
}

#[test]
fn a_table_reply_that_is_applied_yields_only_the_table_notice() {
    let token = fresh_token();
    set_pending_notice(token, "paste.table_no_form");
    assert_eq!(
        resolve_applied(token),
        PendingResolution::Notify("paste.table_no_form", ToastKind::Info)
    );
    // Already taken: a duplicate delivery for the same token raises nothing.
    assert_eq!(resolve_applied(token), PendingResolution::Nothing);
}

#[test]
fn a_successful_conversion_applied_raises_nothing() {
    let token = fresh_token();
    // classify() never calls set_pending_notice when there is no notice key
    // (a plain success, or Empty), so nothing is pending for this token.
    assert_eq!(resolve_applied(token), PendingResolution::Nothing);
}

#[test]
fn discarding_always_raises_the_discard_notice_even_with_nothing_pending() {
    let token = fresh_token();
    assert_eq!(
        resolve_discarded(token),
        PendingResolution::Notify("paste.discarded", ToastKind::Warning)
    );
}

#[test]
fn the_pending_store_evicts_the_oldest_entry_once_it_is_full() {
    let mut store = PendingNotices::default();
    for token in 0..MAX_PENDING_NOTICES as u64 {
        store.set(token, "paste.failed");
    }
    store.set(999, "paste.timed_out"); // one more, over the cap
    assert_eq!(store.take(0), None, "the oldest entry was evicted");
    assert_eq!(store.take(999), Some("paste.timed_out"));
    assert_eq!(
        store.take(1),
        Some("paste.failed"),
        "the next-oldest survived"
    );
}
