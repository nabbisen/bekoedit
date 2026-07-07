//! RFC-032 benchmark: full-reparse-after-mutation latency.
//!
//! Measures the time to build a MarkdownIndex from a synthetic
//! 10 000-word document, validating that MVP full-reparse strategy
//! stays within interactive latency targets.

use bekoedit_markdown::MarkdownIndex;

fn large_doc() -> String {
    let mut s = String::with_capacity(64_000);
    s.push_str("# Performance Benchmark Document\n\n");
    for i in 0..1000 {
        s.push_str(&format!("## Section {i}\n\nThis paragraph contains sample text to reach roughly ten thousand words. It exercises the parser with headings, paragraphs, and lists.\n\n"));
        s.push_str("- item one\n- item two\n- item three\n\n");
        s.push_str(&format!(
            "```rust\nfn example_{i}() {{ println!(\"hello\"); }}\n```\n\n"
        ));
    }
    s
}

fn main() {
    let doc = large_doc();
    let runs = 100u32;
    let start = std::time::Instant::now();
    for rev in 0..runs {
        let _ = MarkdownIndex::build(&doc, rev as u64);
    }
    let elapsed = start.elapsed();
    let per_run_ms = elapsed.as_secs_f64() * 1000.0 / f64::from(runs);
    println!(
        "Full reparse × {runs} runs on {} bytes: {:.2} ms/run",
        doc.len(),
        per_run_ms
    );
    // RFC-032 threshold: < 50 ms per reparse for this document size.
    assert!(
        per_run_ms < 50.0,
        "Reparse too slow: {per_run_ms:.2} ms > 50 ms target (RFC-032)"
    );
    println!("RFC-032: full-reparse adequate for current document sizes ✓");
}
