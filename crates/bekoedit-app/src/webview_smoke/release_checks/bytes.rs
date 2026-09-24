//! #8's byte comparison, taken in Rust against the seeded original -- never
//! through the editor or the parser.

/// The saved file must be the original with the typed `marker` inserted, and
/// nothing else changed: every other byte identical, no line ending
/// normalised. Returns the offset the edit landed at.
///
/// The insertion point is found from the marker in the saved bytes, so the
/// check does not assume where the caret was; it only requires that the edit
/// is one insertion, not inside a CRLF pair, and that the rest is untouched.
pub(super) fn check_saved_bytes(
    scenario: &str,
    original: &[u8],
    saved: &[u8],
    marker: &[u8],
) -> Result<usize, String> {
    if find(original, marker).is_some() {
        return Err(format!(
            "{scenario}: the seeded file already contains the marker"
        ));
    }
    let Some(at) = find(saved, marker) else {
        return Err(format!(
            "{scenario}: the typed marker {:?} is not in the saved file ({} bytes, original {})",
            String::from_utf8_lossy(marker),
            saved.len(),
            original.len()
        ));
    };
    if at > original.len() {
        return Err(format!(
            "{scenario}: the marker is at byte {at}, past the original's {} bytes",
            original.len()
        ));
    }
    if at > 0 && at < original.len() && original[at - 1] == b'\r' && original[at] == b'\n' {
        return Err(format!(
            "{scenario}: the edit landed inside a CRLF pair at byte {at}"
        ));
    }
    let mut expected = original[..at].to_vec();
    expected.extend_from_slice(marker);
    expected.extend_from_slice(&original[at..]);
    if let Some(offset) =
        (0..expected.len().max(saved.len())).find(|&i| expected.get(i) != saved.get(i))
    {
        let show = |bytes: &[u8]| {
            bytes
                .get(offset)
                .map_or("end of file".to_string(), |b| format!("{b:#04x}"))
        };
        return Err(format!(
            "{scenario}: first differing byte offset {offset}: expected {}, saved {} \
             (expected {} bytes, saved {}; the edit itself was at byte {at})",
            show(&expected),
            show(saved),
            expected.len(),
            saved.len()
        ));
    }
    Ok(at)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The file must be byte-for-byte the seeded original (a mode switch with no
/// edit must not write anything). Names the first differing byte offset.
pub(super) fn check_bytes_unchanged(
    scenario: &str,
    original: &[u8],
    saved: &[u8],
) -> Result<(), String> {
    let Some(offset) =
        (0..original.len().max(saved.len())).find(|&i| original.get(i) != saved.get(i))
    else {
        return Ok(());
    };
    let show = |bytes: &[u8]| {
        bytes
            .get(offset)
            .map_or("end of file".to_string(), |b| format!("{b:#04x}"))
    };
    Err(format!(
        "{scenario}: the file changed with no edit: first differing byte offset {offset}: \
         original {}, on disk {} (original {} bytes, on disk {})",
        show(original),
        show(saved),
        original.len(),
        saved.len()
    ))
}
