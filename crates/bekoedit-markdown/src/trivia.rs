//! Source trivia: stylistic source details that must be preserved.
//!
//! Implements the preservation targets of the requirements definition
//! (§10.1 "Preserve Source Trivia", §22.4 `SourceTrivia`): blank lines,
//! indentation, list marker style, ordered numbering style, fence style,
//! and line endings.

use serde::{Deserialize, Serialize};

/// Line ending style of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    Lf,
    Crlf,
    /// Both styles present. Writes preserve the text as-is; no normalization
    /// happens without an explicit user command (requirements §10.2).
    Mixed,
}

impl LineEnding {
    /// Detects the dominant line ending of `text`.
    pub fn detect(text: &str) -> Self {
        let mut lf = 0usize;
        let mut crlf = 0usize;
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' {
                if i > 0 && bytes[i - 1] == b'\r' {
                    crlf += 1;
                } else {
                    lf += 1;
                }
            }
            i += 1;
        }
        match (lf, crlf) {
            (0, 0) | (_, 0) => LineEnding::Lf,
            (0, _) => LineEnding::Crlf,
            _ => LineEnding::Mixed,
        }
    }

    /// The newline sequence to use when generating new lines for this style.
    /// `Mixed` falls back to LF.
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Crlf => "\r\n",
            _ => "\n",
        }
    }
}

/// Bullet list marker style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListMarkerStyle {
    /// `-`
    Dash,
    /// `*`
    Asterisk,
    /// `+`
    Plus,
    /// `1.` style
    NumberDot,
    /// `1)` style
    NumberParen,
}

impl ListMarkerStyle {
    pub fn detect(marker_text: &str) -> Option<Self> {
        let t = marker_text.trim_start();
        let first = t.chars().next()?;
        match first {
            '-' => Some(Self::Dash),
            '*' => Some(Self::Asterisk),
            '+' => Some(Self::Plus),
            c if c.is_ascii_digit() => {
                let rest = t.trim_start_matches(|c: char| c.is_ascii_digit());
                match rest.chars().next() {
                    Some('.') => Some(Self::NumberDot),
                    Some(')') => Some(Self::NumberParen),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Code fence marker style (requirements §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeFenceStyle {
    /// `` ` `` or `~`
    pub marker: char,
    /// Number of marker characters in the opening fence (>= 3).
    pub length: usize,
}

/// Per-block stylistic context preserved across patches (requirements §22.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTrivia {
    pub leading_blank_lines: u16,
    pub trailing_blank_lines: u16,
    /// Indentation (spaces/tabs) of the block's first line.
    pub indentation: String,
    pub list_marker_style: Option<ListMarkerStyle>,
    pub code_fence_style: Option<CodeFenceStyle>,
}

impl SourceTrivia {
    /// Computes trivia for a block spanning `range` within `text`.
    pub fn compute(text: &str, start: usize, end: usize) -> Self {
        let before = &text[..start];
        let after = &text[end.min(text.len())..];

        let leading_blank_lines = count_adjacent_blank_lines(before.lines().rev());
        let trailing_blank_lines = count_adjacent_blank_lines(after.lines());

        let first_line = text[start..].lines().next().unwrap_or("");
        let indentation: String = first_line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();

        Self {
            leading_blank_lines,
            trailing_blank_lines,
            indentation,
            list_marker_style: None,
            code_fence_style: None,
        }
    }
}

fn count_adjacent_blank_lines<'a>(lines: impl Iterator<Item = &'a str>) -> u16 {
    let mut n = 0u16;
    for line in lines {
        if line.trim().is_empty() {
            n = n.saturating_add(1);
        } else {
            break;
        }
    }
    n
}
