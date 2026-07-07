//! Rust-owned UTF-8 byte ranges into canonical Markdown text.
//!
//! Per RFC-013 and the requirements definition (§9.7), byte ranges are
//! always resolved and validated by the Rust core. Ranges originating
//! from the UI (UTF-16 based editors) are advisory only.

use serde::{Deserialize, Serialize};

/// A half-open byte range `[start, end)` into canonical UTF-8 text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub fn contains(&self, other: &ByteRange) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Validates that the range is well-formed, inside `text`, and that both
    /// boundaries lie on UTF-8 character boundaries.
    ///
    /// This is the single gate through which every source mutation must pass
    /// (RFC-015 safety invariant: invalid UTF-8 boundary patches must be
    /// impossible).
    pub fn validate(&self, text: &str) -> Result<(), RangeError> {
        if self.start > self.end {
            return Err(RangeError::Inverted {
                start: self.start,
                end: self.end,
            });
        }
        if self.end > text.len() {
            return Err(RangeError::OutOfBounds {
                end: self.end,
                len: text.len(),
            });
        }
        if !text.is_char_boundary(self.start) {
            return Err(RangeError::NotCharBoundary { offset: self.start });
        }
        if !text.is_char_boundary(self.end) {
            return Err(RangeError::NotCharBoundary { offset: self.end });
        }
        Ok(())
    }

    /// Returns the slice of `text` covered by this range after validation.
    pub fn slice<'a>(&self, text: &'a str) -> Result<&'a str, RangeError> {
        self.validate(text)?;
        Ok(&text[self.start..self.end])
    }
}

/// Converts a UTF-16 code-unit offset (as reported by browser JS APIs such
/// as `selectionStart` / `selectionEnd`) to a UTF-8 byte position within
/// `text`.
///
/// Returns `None` if `utf16_offset` falls in the middle of a surrogate pair
/// or exceeds the text's UTF-16 length — both indicate a client-side bug.
/// Callers must treat `None` as a safe no-op, never a panic.
pub fn utf16_to_utf8_offset(text: &str, utf16_offset: usize) -> Option<usize> {
    let mut byte_pos = 0usize;
    let mut utf16_pos = 0usize;
    for ch in text.chars() {
        if utf16_pos == utf16_offset {
            return Some(byte_pos);
        }
        // A surrogate pair spans 2 UTF-16 code units; if the offset lands
        // inside one, it cannot be a valid boundary — return None.
        if utf16_pos > utf16_offset {
            return None;
        }
        utf16_pos += ch.len_utf16();
        byte_pos += ch.len_utf8();
    }
    (utf16_pos == utf16_offset).then_some(byte_pos)
}

impl From<std::ops::Range<usize>> for ByteRange {
    fn from(r: std::ops::Range<usize>) -> Self {
        Self::new(r.start, r.end)
    }
}

/// Validation failures for byte ranges.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum RangeError {
    #[error("range start {start} is greater than end {end}")]
    Inverted { start: usize, end: usize },
    #[error("range end {end} exceeds text length {len}")]
    OutOfBounds { end: usize, len: usize },
    #[error("offset {offset} is not a UTF-8 character boundary")]
    NotCharBoundary { offset: usize },
}
