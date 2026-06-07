//! RFC-015 acceptance criteria: validated application, revision rejection,
//! and impossibility of invalid UTF-8 boundary mutation.

use crate::patch::{PatchError, PatchOrigin, SourcePatch, apply_patch};
use crate::range::{ByteRange, RangeError};

fn patch(base: u64, start: usize, end: usize, replacement: &str) -> SourcePatch {
    SourcePatch {
        base_revision: base,
        range: ByteRange::new(start, end),
        replacement: replacement.to_string(),
        origin: PatchOrigin::FormMode,
    }
}

#[test]
fn applies_minimal_replacement() {
    let mut text = String::from("hello world");
    let result = apply_patch(&mut text, 1, &patch(1, 6, 11, "bekoedit")).unwrap();
    assert_eq!(text, "hello bekoedit");
    assert_eq!(result.affected_range, ByteRange::new(6, 14));
    assert!(result.reparse_required);
}

#[test]
fn rejects_stale_revision() {
    let mut text = String::from("hello");
    let err = apply_patch(&mut text, 2, &patch(1, 0, 1, "H")).unwrap_err();
    assert_eq!(
        err,
        PatchError::DocumentRevisionMismatch {
            base: 1,
            current: 2
        }
    );
    assert_eq!(text, "hello", "rejected patch must not mutate text");
}

#[test]
fn rejects_out_of_bounds_range() {
    let mut text = String::from("short");
    let err = apply_patch(&mut text, 1, &patch(1, 0, 99, "x")).unwrap_err();
    assert!(matches!(
        err,
        PatchError::InvalidRange(RangeError::OutOfBounds { .. })
    ));
}

#[test]
fn rejects_non_char_boundary_in_multibyte_text() {
    // "日" is 3 bytes; offset 1 splits the character.
    let mut text = String::from("日本語");
    let err = apply_patch(&mut text, 1, &patch(1, 1, 3, "x")).unwrap_err();
    assert!(matches!(
        err,
        PatchError::InvalidRange(RangeError::NotCharBoundary { offset: 1 })
    ));
    assert_eq!(text, "日本語");
}

#[test]
fn multibyte_safe_replacement_succeeds() {
    let mut text = String::from("こんにちは世界");
    // Replace "世界" (starts at byte 15) with "bekoedit".
    apply_patch(&mut text, 1, &patch(1, 15, 21, "bekoedit")).unwrap();
    assert_eq!(text, "こんにちはbekoedit");
}

#[test]
fn inverted_range_is_rejected() {
    let mut text = String::from("abc");
    let err = apply_patch(&mut text, 1, &patch(1, 2, 1, "x")).unwrap_err();
    assert!(matches!(
        err,
        PatchError::InvalidRange(RangeError::Inverted { .. })
    ));
}
