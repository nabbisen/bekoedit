//! A small module of its own so `tests.rs` stays under the ELOC gate
//! (RFC-044 slice-1 handoff §9: "prefer a new module").

/// True iff, somewhere in `rust`, the line creating the driver eval is
/// followed (skipping blank lines) by a line that is a bare `loop`.
/// Indentation-insensitive on purpose: the original guard hardcoded twelve
/// leading spaces, matching webview_smoke.rs's old nesting depth; the
/// extraction (RFC-044 slice-1 §3) moved this into a top-level function at
/// four spaces, so that literal string could never match again -- the
/// guard looked repointed but had stopped guarding anything.
pub(super) fn eval_immediately_followed_by_a_loop(rust: &str) -> bool {
    let mut lines = rust.lines().peekable();
    while let Some(line) = lines.next() {
        if !line
            .trim_start()
            .starts_with("let mut eval = document::eval(")
        {
            continue;
        }
        let next = lines.clone();
        for candidate in next {
            let trimmed = candidate.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "loop" || trimmed.starts_with("loop {") || trimmed.starts_with("loop{") {
                return true;
            }
            break;
        }
    }
    false
}

#[test]
fn eval_immediately_followed_by_a_loop_detects_the_anti_pattern_at_any_indentation() {
    assert!(!eval_immediately_followed_by_a_loop(
        "    let mut eval = document::eval(driver_js);\n    eval.send(x);"
    ));
    assert!(eval_immediately_followed_by_a_loop(
        "            let mut eval = document::eval(driver_js);\n            loop {"
    ));
    assert!(eval_immediately_followed_by_a_loop(
        "let mut eval = document::eval(driver_js);\nloop"
    ));
    assert!(eval_immediately_followed_by_a_loop(
        "    let mut eval = document::eval(driver_js);\n\n    loop {"
    ));
}
