//! RFC-050 slice 1, level 2: the accessibility markup of the shell, checked
//! headless, in both languages, on every CI run. Level 3 (real WebKit through
//! AT-SPI) is `scripts/a11y-observer.py`; this module holds what the markup alone
//! can prove.
//!
//! Expected names come from `i18n.rs`'s own lookup (`tr`), never from strings
//! typed here, so a blank label, or a label in the wrong language, fails.

mod checks;
mod harness;
pub(crate) mod html;
#[cfg(test)]
mod html_tests;
