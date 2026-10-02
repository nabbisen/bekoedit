use super::*;
use form_commit::CommitOutcome;

/// Task 048 §2.2: only `Composing` must refuse the command. A mutation
/// that makes this always `false` (composing proceeds again, exactly the
/// defect the review found) is what this test exists to catch.
#[test]
fn only_composing_refuses_the_command() {
    assert!(should_refuse(CommitOutcome::Composing));
    assert!(!should_refuse(CommitOutcome::Committed));
    assert!(!should_refuse(CommitOutcome::NothingPending));
}
