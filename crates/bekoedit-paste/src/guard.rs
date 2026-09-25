//! The `data:` guard (RFC-046 §5.3, §10 Q4).
//!
//! A pasted screenshot arrives as `![alt](data:image/png;base64,…)`, megabytes
//! of base64 in a place the user is expected to read and edit. This replaces
//! any image or link whose destination is a `data:` URI with what a reader
//! would have seen: the alt text (or a marker, when an image has none) for an
//! image, the link text for a link.
//!
//! **What it parses, and why that is enough.** `mdka` is the only producer of
//! this Markdown, and it writes links and images in one shape, `[label](dest
//! "title")` or `![alt](dest "title")`, escaping brackets in the label and
//! wrapping a destination that needs it in `<…>`. So this reads exactly that
//! shape, and no more Markdown than is needed to avoid touching code:
//!
//! - **fenced code blocks** (` ``` ` or `~~~`, possibly inside a blockquote or a
//!   list item) are copied through untouched;
//! - **inline code spans** are copied through untouched;
//! - a **backslash escape** is copied through, so `\![` is not an image;
//! - a bracket pair that is not followed by `(dest…)` is not a link.
//!
//! It is not a CommonMark parser, and the proof that it does not need to be is
//! `tests/corpus.rs`, which parses the guarded output with `pulldown-cmark` and
//! fails on any `data:` destination that survives, and on any change to code
//! content.

/// See the module docs.
pub(crate) fn strip_data_destinations(markdown: &str, marker: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut fence: Option<Fence> = None;
    for line in markdown.split_inclusive('\n') {
        let container_free = after_containers(line);
        match &fence {
            Some(open) => {
                if closes(open, container_free) {
                    fence = None;
                }
                out.push_str(line);
            }
            None => {
                if let Some(open) = opens(container_free) {
                    fence = Some(open);
                    out.push_str(line);
                } else {
                    scan(line, marker, &mut out);
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
struct Fence {
    ch: u8,
    len: usize,
}

/// The line with blockquote markers, indentation and one list marker per level
/// removed, so a fence inside a quote or a list item is found.
fn after_containers(line: &str) -> &str {
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start_matches([' ', '\t']);
        if let Some(after) = trimmed.strip_prefix('>') {
            rest = after;
            continue;
        }
        let bytes = trimmed.as_bytes();
        if matches!(bytes.first(), Some(b'-' | b'*' | b'+'))
            && matches!(bytes.get(1), Some(b' ' | b'\t'))
        {
            rest = &trimmed[2..];
            continue;
        }
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if (1..=9).contains(&digits)
            && matches!(bytes.get(digits), Some(b'.' | b')'))
            && matches!(bytes.get(digits + 1), Some(b' ' | b'\t'))
        {
            rest = &trimmed[digits + 2..];
            continue;
        }
        return trimmed;
    }
}

fn run_of(text: &str, ch: u8) -> usize {
    text.bytes().take_while(|&b| b == ch).count()
}

/// A fence that opens on this line, if any. A backtick fence's info string may
/// not contain a backtick (otherwise it is an inline code span).
fn opens(container_free: &str) -> Option<Fence> {
    let ch = *container_free.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = run_of(container_free, ch);
    if len < 3 {
        return None;
    }
    let info = &container_free[len..];
    if ch == b'`' && info.contains('`') {
        return None;
    }
    Some(Fence { ch, len })
}

fn closes(open: &Fence, container_free: &str) -> bool {
    let len = run_of(container_free, open.ch);
    len >= open.len && container_free[len..].trim().is_empty()
}

/// Copies `text` to `out`, replacing every `data:` link or image outside code.
fn scan(text: &str, marker: &str, out: &mut String) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                let end = next_boundary(text, i + 1);
                out.push_str(&text[i..end]);
                i = end;
            }
            b'`' => {
                let end = code_span_end(text, i).unwrap_or_else(|| i + run_of(&text[i..], b'`'));
                out.push_str(&text[i..end]);
                i = end;
            }
            b'!' if bytes.get(i + 1) == Some(&b'[') => {
                if let Some(link) = parse_link(text, i + 1) {
                    push_construct(text, i, &link, true, marker, out);
                    i = link.end;
                } else {
                    out.push('!');
                    i += 1;
                }
            }
            b'[' => {
                if let Some(link) = parse_link(text, i) {
                    push_construct(text, i, &link, false, marker, out);
                    i = link.end;
                } else {
                    out.push('[');
                    i += 1;
                }
            }
            _ => {
                let end = next_boundary(text, i);
                out.push_str(&text[i..end]);
                i = end;
            }
        }
    }
}

/// One `[label](destination "title")`, as byte offsets into the scanned text.
struct Link {
    /// The label, without its brackets.
    label: (usize, usize),
    /// The destination, without any `<…>`.
    destination: (usize, usize),
    /// Just past the closing `)`.
    end: usize,
}

fn push_construct(
    text: &str,
    start: usize,
    link: &Link,
    is_image: bool,
    marker: &str,
    out: &mut String,
) {
    let label = &text[link.label.0..link.label.1];
    let destination = &text[link.destination.0..link.destination.1];
    let is_data = destination
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"));
    if is_data {
        if is_image && label.trim().is_empty() {
            out.push_str(marker);
        } else {
            // The label may itself hold a nested link or image.
            scan(label, marker, out);
        }
        return;
    }
    // Not a data: destination: keep the construct, but look inside its label,
    // where an image with a data: source may be nested in a link.
    let close_bracket = link.label.1;
    if is_image {
        out.push('!');
    }
    out.push_str(&text[start + usize::from(is_image)..link.label.0]);
    scan(label, marker, out);
    out.push_str(&text[close_bracket..link.end]);
}

/// The end of the char that starts at `i` (or the end of the text).
fn next_boundary(text: &str, i: usize) -> usize {
    let mut end = (i + 1).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    end
}

/// The end of the code span opened by the backtick run at `start`, if one
/// closes on this line: a later run of exactly the same length.
fn code_span_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = run_of(&text[start..], b'`');
    let mut i = start + len;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = run_of(&text[i..], b'`');
            if run == len {
                return Some(i + run);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

/// Parses `[label](destination "title")` with `[` at `open`.
fn parse_link(text: &str, open: usize) -> Option<Link> {
    let bytes = text.as_bytes();
    // The label: bracket-balanced, skipping escapes and code spans.
    let mut depth = 0usize;
    let mut i = open;
    let close = loop {
        match *bytes.get(i)? {
            b'\\' => i = next_boundary(text, i + 1),
            b'`' => {
                i = code_span_end(text, i).unwrap_or_else(|| i + run_of(&text[i..], b'`'));
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
                i += 1;
            }
            _ => i += 1,
        }
    };
    if bytes.get(close + 1) != Some(&b'(') {
        return None;
    }
    let mut i = skip_space(bytes, close + 2);
    let destination = if bytes.get(i) == Some(&b'<') {
        let start = i + 1;
        let mut j = start;
        loop {
            match *bytes.get(j)? {
                b'\\' => j = next_boundary(text, j + 1),
                b'>' => break,
                b'\n' | b'<' => return None,
                _ => j += 1,
            }
        }
        i = j + 1;
        (start, j)
    } else {
        let start = i;
        let mut parens = 0usize;
        while let Some(&b) = bytes.get(i) {
            match b {
                b'\\' => i = next_boundary(text, i + 1),
                b'(' => {
                    parens += 1;
                    i += 1;
                }
                b')' if parens == 0 => break,
                b')' => {
                    parens -= 1;
                    i += 1;
                }
                b' ' | b'\t' | b'\n' => break,
                _ => i += 1,
            }
        }
        (start, i)
    };
    i = skip_space(bytes, i);
    if let Some(&quote) = bytes.get(i).filter(|b| matches!(b, b'"' | b'\'' | b'(')) {
        let closer = if quote == b'(' { b')' } else { quote };
        i += 1;
        loop {
            match *bytes.get(i)? {
                b'\\' => i = next_boundary(text, i + 1),
                b if b == closer => break,
                _ => i += 1,
            }
        }
        i = skip_space(bytes, i + 1);
    }
    (bytes.get(i) == Some(&b')')).then_some(Link {
        label: (open + 1, close),
        destination,
        end: i + 1,
    })
}

fn skip_space(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    i
}
