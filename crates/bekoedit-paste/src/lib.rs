//! Clipboard HTML to Markdown for bekoedit's paste path (RFC-046 slice 1).
//!
//! A pure, headless converter. It turns the `text/html` flavour of a paste into
//! Markdown, or says why it declined, and never panics, never blocks past a
//! fixed budget, and never returns a `data:` destination or a line ending other
//! than the one asked for. Nothing here touches the app: the paste handler,
//! the toast and the editor transaction are slice 2.
//!
//! Conversion is `mdka` 2.5.1 in `ConversionMode::Minimal` with default
//! features off (RFC-046 §5.3, §6.2). Around it:
//!
//! - **Too large** (`MAX_HTML_BYTES`), measured before `mdka` is called.
//! - **Failed**: `mdka` panics. The panic is caught, so a paste can never take
//!   the app down. (`mdka`'s string API has no error return, so a panic is its
//!   only failure.)
//! - **Timed out** (`CONVERSION_BUDGET`): the conversion runs on a worker
//!   thread and is waited for with a bound. `mdka` is CPU-bound and cannot be
//!   cancelled, so a stalled worker **keeps running until `mdka` returns**; its
//!   result is dropped when it finally arrives (the channel's receiver is gone),
//!   and nothing else holds a reference to it. Each timed-out paste therefore
//!   costs one thread and one copy of the HTML until then.
//! - **Empty**: the output is blank.
//!
//! On success, two guards run over `mdka`'s output: a `data:` destination
//! (image or link) outside code is replaced by its alt or link text, or by a
//! marker when an image has no alt (RFC-046 §10 Q4); and every line break
//! becomes the target line ending. A third rule, that the only raw HTML in the
//! output is a bare `<br>` inside a table cell, is *not* enforced by rewriting:
//! it is `mdka`'s documented behaviour, and the fixture corpus checks it
//! structurally (`tests/corpus.rs`).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

mod guard;

/// The HTML flavour is not converted above this many bytes (1 MiB). Measured on
/// the raw HTML, before `mdka` runs (RFC-046 §5.4 rule 1).
pub const MAX_HTML_BYTES: usize = 1024 * 1024;

/// How long a conversion may take before the plain flavour wins (RFC-046 §5.4
/// rule 2). A 1 MiB paste converts in tens of milliseconds, so this is reached
/// only by a genuine stall.
pub const CONVERSION_BUDGET: Duration = Duration::from_secs(2);

/// What replaces a `data:` image that has no alt text. Slice 2 passes a
/// localized one through [`convert_with_marker`].
pub const DATA_IMAGE_MARKER: &str = "(image omitted)";

/// The line ending to write: the target document's. A document with mixed line
/// endings is slice 2's decision to map onto one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// Why the HTML flavour was not used. Slice 2 inserts the plain flavour and
/// raises one toast naming this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// Over [`MAX_HTML_BYTES`].
    TooLarge,
    /// `mdka` panicked, or its worker could not be started.
    Failed,
    /// Over [`CONVERSION_BUDGET`].
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Converted {
        markdown: String,
        /// The HTML contained `<table` (ASCII case-insensitive). Slice 2 raises
        /// "pasted table as text: no Markdown table form" when this is true and
        /// the output has no GFM table (`bekoedit_markdown::has_gfm_table`).
        html_had_table: bool,
    },
    /// Insert the plain flavour, and say why (RFC-046 §5.4).
    Fallback(FallbackReason),
    /// The output was blank: insert the plain flavour, quietly (§5.4 rule 4).
    /// Also returned when the plain flavour is blank too; then there is
    /// nothing to insert either way.
    Empty,
}

/// Failures of the guarded conversion, before they are folded into a
/// [`FallbackReason`].
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("the HTML is {size} bytes, over the {limit}-byte limit")]
    TooLarge { size: usize, limit: usize },
    #[error("mdka panicked: {0}")]
    Panicked(String),
    #[error("the conversion did not finish within {0:?}")]
    TimedOut(Duration),
    #[error("could not start the conversion worker: {0}")]
    Spawn(String),
}

impl From<&ConvertError> for FallbackReason {
    fn from(error: &ConvertError) -> Self {
        match error {
            ConvertError::TooLarge { .. } => Self::TooLarge,
            ConvertError::TimedOut(_) => Self::TimedOut,
            ConvertError::Panicked(_) | ConvertError::Spawn(_) => Self::Failed,
        }
    }
}

/// Converts clipboard HTML to Markdown in the target line ending. `plain` is
/// the `text/plain` flavour, used only to decide whether a blank result is
/// worth reporting.
pub fn convert(html: &str, plain: &str, line_ending: LineEnding) -> Outcome {
    convert_with_marker(html, plain, line_ending, DATA_IMAGE_MARKER)
}

/// [`convert`], with the text that replaces an alt-less `data:` image.
pub fn convert_with_marker(
    html: &str,
    plain: &str,
    line_ending: LineEnding,
    image_marker: &str,
) -> Outcome {
    convert_using(
        html,
        plain,
        line_ending,
        image_marker,
        Limits::DEFAULT,
        mdka_minimal,
    )
}

/// `mdka` as configured for paste: `Minimal`, which drops page chrome and emits
/// no `<a id>` anchors (RFC-046 §5.3).
fn mdka_minimal(html: String) -> String {
    let options = mdka::ConversionOptions::for_mode(mdka::ConversionMode::Minimal);
    mdka::html_to_markdown_with(&html, &options)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub max_bytes: usize,
    pub budget: Duration,
}

impl Limits {
    pub(crate) const DEFAULT: Self = Self {
        max_bytes: MAX_HTML_BYTES,
        budget: CONVERSION_BUDGET,
    };
}

/// The whole pipeline, with the limits and the engine given, so each rule is
/// tested without a real stall or a real panic in `mdka`.
pub(crate) fn convert_using<F>(
    html: &str,
    plain: &str,
    line_ending: LineEnding,
    image_marker: &str,
    limits: Limits,
    engine: F,
) -> Outcome
where
    F: FnOnce(String) -> String + Send + 'static,
{
    let converted = match run_guarded(html, limits, engine) {
        Ok(markdown) => markdown,
        Err(error) => return Outcome::Fallback(FallbackReason::from(&error)),
    };
    let markdown = guard::strip_data_destinations(&converted, image_marker);
    let markdown = normalize_line_endings(&markdown, line_ending);
    if markdown.trim().is_empty() {
        // `plain` is not consulted: a blank result is `Empty` either way. It is
        // a parameter so the contract can grow without an API change.
        let _ = plain;
        return Outcome::Empty;
    }
    Outcome::Converted {
        markdown,
        html_had_table: contains_table(html),
    }
}

fn run_guarded<F>(html: &str, limits: Limits, engine: F) -> Result<String, ConvertError>
where
    F: FnOnce(String) -> String + Send + 'static,
{
    if html.len() > limits.max_bytes {
        return Err(ConvertError::TooLarge {
            size: html.len(),
            limit: limits.max_bytes,
        });
    }
    let (sender, receiver) = mpsc::channel();
    let input = html.to_owned();
    thread::Builder::new()
        .name("bekoedit-paste-convert".into())
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| engine(input)));
            // The receiver is gone if the caller timed out; the result is then
            // dropped here, which is the point.
            let _ = sender.send(result);
        })
        .map_err(|error| ConvertError::Spawn(error.to_string()))?;
    match receiver.recv_timeout(limits.budget) {
        Ok(Ok(markdown)) => Ok(markdown),
        Ok(Err(panic)) => Err(ConvertError::Panicked(panic_message(&*panic))),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(ConvertError::TimedOut(limits.budget)),
        // The worker ended without sending: it is not a stall, it is a failure.
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(ConvertError::Panicked(
            "the conversion worker ended without a result".to_string(),
        )),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a non-string panic payload".to_string())
}

/// Every line break, `\r\n`, a lone `\r` or `\n`, becomes `line_ending`.
pub(crate) fn normalize_line_endings(text: &str, line_ending: LineEnding) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str(line_ending.as_str());
            }
            '\n' => out.push_str(line_ending.as_str()),
            other => out.push(other),
        }
    }
    out
}

/// `<table`, ASCII case-insensitive (RFC-046 §5.4).
pub(crate) fn contains_table(html: &str) -> bool {
    const NEEDLE: &[u8] = b"<table";
    html.as_bytes()
        .windows(NEEDLE.len())
        .any(|window| window.eq_ignore_ascii_case(NEEDLE))
}

#[cfg(test)]
mod tests;
