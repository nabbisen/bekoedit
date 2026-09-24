//! Reconciling the source editor's text with the session's canonical text
//! (task 027). The one place this happens.
//!
//! The editor speaks **editor form**: every line break is `\n`, exactly as
//! CodeMirror's `doc.toString()` returns it, whatever the file contains. The
//! session keeps **canonical form**: the file's own bytes. Text Mode used to
//! replace the canonical text with the editor's text wholesale, which turned
//! every CRLF into LF on the first snapshot -- on save, and at every mode
//! switch, edit or no edit.
//!
//! - **Rule 0.** Text equal to the editor form of the canonical text changes
//!   nothing: no revision, no dirty flag, no re-detection.
//! - **Rule 1, uniform files** (only `\n`, or only `\r\n`, and no lone `\r`):
//!   the new canonical text is the incoming text with every `\n` replaced by
//!   the file's ending. Exact for every edit, including multi-location ones.
//! - **Rule 2, mixed files or any lone `\r`:** a minimal splice. The common
//!   prefix and suffix of the two forms are found, both boundaries are mapped
//!   back to canonical byte offsets through the line-break map (a boundary
//!   never splits a `\r\n`, and always falls on a character boundary), and
//!   only that range is replaced. Line breaks inside the inserted text take
//!   the ending of the canonical break that ends the line where the splice
//!   starts, else the break before it (the last line), else `\n`.
//!
//! **Residual, accepted:** in a *mixed* file, one snapshot carrying edits at
//! several separated places can re-end the untouched lines *between* them,
//! because the splice spans from the first change to the last. A single edit
//! changes only the lines it touches.

/// The editor form of `text`: every `\r\n`, then every lone `\r`, becomes
/// `\n` -- exactly what CodeMirror does when it splits a document into lines.
pub fn editor_form(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// The editor position (CodeMirror's: UTF-16 code units into the editor form)
/// of a canonical byte offset -- for handing a position from Rust to the
/// editor. A position between the `\r` and `\n` of one break maps to before
/// that break, since the editor has no position there.
pub fn editor_position(canonical: &str, byte_offset: usize) -> usize {
    let mut end = byte_offset.min(canonical.len());
    while !canonical.is_char_boundary(end) {
        end -= 1;
    }
    if canonical[..end].ends_with('\r') && canonical[end..].starts_with('\n') {
        end -= 1;
    }
    editor_form(&canonical[..end]).encode_utf16().count()
}

/// One line break in canonical text.
#[derive(Debug, Clone, Copy)]
struct Break {
    /// Its byte offset in canonical text.
    at: usize,
    /// Its length there: 2 for `\r\n`, else 1.
    len: usize,
    /// Its byte offset in the editor form.
    editor_at: usize,
}

fn breaks(canonical: &str) -> Vec<Break> {
    let bytes = canonical.as_bytes();
    let mut out = Vec::new();
    let mut shift = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                out.push(Break {
                    at: i,
                    len: 2,
                    editor_at: i - shift,
                });
                shift += 1;
                i += 2;
            }
            b'\r' | b'\n' => {
                out.push(Break {
                    at: i,
                    len: 1,
                    editor_at: i - shift,
                });
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// A canonical byte offset for an editor-form byte offset: every break wholly
/// before it contributes its extra bytes.
fn canonical_offset(breaks: &[Break], editor_offset: usize) -> usize {
    editor_offset
        + breaks
            .iter()
            .filter(|b| b.editor_at < editor_offset)
            .map(|b| b.len - 1)
            .sum::<usize>()
}

/// The new canonical text for `incoming` editor text over `canonical`.
/// Returns `canonical` itself, unchanged, under Rule 0.
pub fn reconcile(canonical: &str, incoming: &str) -> String {
    let incoming = editor_form(incoming);
    let current = editor_form(canonical);
    if incoming == current {
        return canonical.to_string();
    }
    let breaks = breaks(canonical);
    let is = |b: &Break, byte: u8| canonical.as_bytes()[b.at] == byte;
    let crlf = breaks.iter().filter(|b| b.len == 2).count();
    let lone_cr = breaks.iter().filter(|b| b.len == 1 && is(b, b'\r')).count();
    let lf = breaks.iter().filter(|b| b.len == 1 && is(b, b'\n')).count();
    if lone_cr == 0 && (crlf == 0 || lf == 0) {
        return incoming; // THROWAWAY MUTATION: Rule 1 replaced by "keep the incoming text"
    }

    let prefix: usize = current
        .chars()
        .zip(incoming.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let suffix: usize = current[prefix..]
        .chars()
        .rev()
        .zip(incoming[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let start = canonical_offset(&breaks, prefix);
    let end = canonical_offset(&breaks, current.len() - suffix);
    let ending = breaks
        .iter()
        .find(|b| b.at >= start)
        .or(breaks.last())
        .map_or("\n", |b| &canonical[b.at..b.at + b.len]);
    let inserted = incoming[prefix..incoming.len() - suffix].replace('\n', ending);
    format!("{}{inserted}{}", &canonical[..start], &canonical[end..])
}
