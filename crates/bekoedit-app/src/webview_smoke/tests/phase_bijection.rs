//! A small module of its own so `tests.rs` stays under the ELOC guideline
//! (task 025 §5, mirroring `transport_guard.rs`'s own reason). Task 025
//! §2.2's bijection test for RFC-041's run: the same shape as
//! `shell_behaviour/tests/phase_bijection.rs`'s and
//! `trusted_click/tests.rs`'s, for this run's three phases.

use std::collections::BTreeSet;

use crate::webview_smoke::WEBVIEW_SMOKE_JS;
use crate::webview_smoke::protocol::SmokePhase;
use crate::webview_smoke::transport::{parse_js_declared_phase_list, phases_via_next};

/// Every phase variant, matched exhaustively with no wildcard: adding a
/// variant without adding it here is a compile error, naming it -- paired
/// with the derived walk below (which only proves reachability via
/// `next()`, not that every declared variant was swept into it).
const fn phase_count() -> usize {
    match SmokePhase::Launch {
        SmokePhase::Launch | SmokePhase::Editor | SmokePhase::Preview => {}
    }
    3
}

fn walked() -> BTreeSet<&'static str> {
    phases_via_next(SmokePhase::Launch, SmokePhase::next, phase_count())
        .into_iter()
        .map(SmokePhase::as_str)
        .collect()
}

#[test]
fn every_variant_is_reached_by_walking_next_from_the_first_phase() {
    let walked = walked();
    assert_eq!(
        walked.len(),
        phase_count(),
        "next() walk visited {} phases but phase_count() says there are {} -- \
         a variant exists that next() never reaches",
        walked.len(),
        phase_count()
    );
}

/// Task 025 §2.2: `SmokePhase::as_str()` must match driver.js's own
/// `phases` array exactly -- RFC-041's own version of task 023's bug,
/// never actually latent here (`as_str()` already returns each phase's own
/// name), but unguarded until this test existed.
#[test]
fn every_as_str_is_a_phase_the_driver_knows() {
    let driver_phases = parse_js_declared_phase_list(WEBVIEW_SMOKE_JS);
    let rust_phases = walked();
    assert_eq!(
        rust_phases, driver_phases,
        "SmokePhase::as_str() must match driver.js's own phases array exactly -- \
         a mismatch is a request the driver rejects before its own try/catch, \
         silently, for the shared transport's full 5 s round-trip cap"
    );
}
