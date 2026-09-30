use super::*;
use bekoedit_ui_contract::source_editor::{EditorInstanceId, SourceEditorId, SourceEpoch};

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

fn identity(instance: u64) -> EditorIdentity {
    EditorIdentity {
        instance_id: EditorInstanceId::new(instance),
        editor_id: SourceEditorId::Text,
        document_id: 1,
        epoch: SourceEpoch::new(1),
    }
}

#[test]
fn a_fallback_reply_that_is_then_discarded_yields_only_the_discard_notice() {
    let editor = identity(1);
    let token = fresh_token();
    set_pending_notice(editor, token, "paste.failed");
    assert_eq!(
        resolve_discarded(editor, token),
        PendingResolution::Notify("paste.discarded", ToastKind::Warning)
    );
    // The dropped reply notice ("pasted as plain text: could not convert")
    // cannot surface later either -- nothing was actually pasted.
    assert_eq!(resolve_applied(editor, token), PendingResolution::Nothing);
}

#[test]
fn a_table_reply_that_is_applied_yields_only_the_table_notice() {
    let editor = identity(2);
    let token = fresh_token();
    set_pending_notice(editor, token, "paste.table_no_form");
    assert_eq!(
        resolve_applied(editor, token),
        PendingResolution::Notify("paste.table_no_form", ToastKind::Info)
    );
    // Already taken: a duplicate delivery for the same paste raises nothing.
    assert_eq!(resolve_applied(editor, token), PendingResolution::Nothing);
}

#[test]
fn a_successful_conversion_applied_raises_nothing() {
    let editor = identity(3);
    let token = fresh_token();
    // classify() never calls set_pending_notice when there is no notice key
    // (a plain success, or Empty), so nothing is pending for this paste.
    assert_eq!(resolve_applied(editor, token), PendingResolution::Nothing);
}

#[test]
fn discarding_always_raises_the_discard_notice_even_with_nothing_pending() {
    let editor = identity(4);
    let token = fresh_token();
    assert_eq!(
        resolve_discarded(editor, token),
        PendingResolution::Notify("paste.discarded", ToastKind::Warning)
    );
}

/// Required by review (2026-09-30): a token number is only unique within one
/// editor script instance (`paste.js`'s `nextToken` restarts on a remount),
/// so the store must not let identity A's entry be found under identity B's
/// same token number.
#[test]
fn a_stale_entry_for_one_identity_is_not_taken_by_another_identity_s_same_token() {
    let a = identity(5);
    let b = identity(6);
    let token = fresh_token();
    set_pending_notice(a, token, "paste.failed");
    // B's apply for the same token number finds nothing of A's.
    assert_eq!(resolve_applied(b, token), PendingResolution::Nothing);
    // B's discard for the same token number raises only the discard notice.
    assert_eq!(
        resolve_discarded(b, token),
        PendingResolution::Notify("paste.discarded", ToastKind::Warning)
    );
    // A's own entry is still there, untouched by B's lookups.
    assert_eq!(
        resolve_applied(a, token),
        PendingResolution::Notify("paste.failed", ToastKind::Info)
    );
}

#[test]
fn the_pending_store_evicts_the_oldest_entry_once_it_is_full() {
    let editor = identity(7);
    let mut store = PendingNotices::default();
    for token in 0..MAX_PENDING_NOTICES as u64 {
        store.set(editor, token, "paste.failed");
    }
    store.set(editor, 999, "paste.timed_out"); // one more, over the cap
    assert_eq!(store.take(editor, 0), None, "the oldest entry was evicted");
    assert_eq!(store.take(editor, 999), Some("paste.timed_out"));
    assert_eq!(
        store.take(editor, 1),
        Some("paste.failed"),
        "the next-oldest survived"
    );
}
