//! Inline formatting toggle resolution (RFC-030).
//!
//! Converts `ToggleInline` commands with UTF-16 selection offsets into
//! minimal source patches that wrap or unwrap `**bold**`, `_italic_`,
//! `` `code` ``, and `[link](url)` around the selected text.

use crate::block::BlockNode;
use crate::form::{FormEditError, InlineFormat};
use crate::patch::PatchOrigin;
use crate::range::utf16_to_utf8_offset;

// Type alias matching resolve.rs convention.
type Resolved = (crate::range::ByteRange, String, crate::patch::PatchOrigin);

fn require_editable(block: &crate::block::BlockNode) -> Result<(), crate::form::FormEditError> {
    if block.editable_policy == crate::block::EditablePolicy::FormEditable {
        Ok(())
    } else {
        Err(crate::form::FormEditError::UnsupportedEditOperation {
            reason: "block is not form-editable".into(),
        })
    }
}

/// The longest run of consecutive backticks anywhere in `text`.
fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// Wraps `selected` in a code span the way CommonMark requires (task 047
/// Part C): the backtick fence is one longer than the longest backtick run
/// already inside `selected`, so the fence can never be mistaken for part
/// of the content, and a single space pads each side when `selected`
/// itself begins or ends with a backtick, so that backtick is never read
/// as touching the fence.
fn wrap_code(selected: &str) -> String {
    let fence = "`".repeat(longest_backtick_run(selected) + 1);
    if selected.starts_with('`') || selected.ends_with('`') {
        format!("{fence} {selected} {fence}")
    } else {
        format!("{fence}{selected}{fence}")
    }
}

/// The inverse of [`wrap_code`]: `None` if `selected` is not a backtick
/// span at all (no matching, nonzero backtick run at both ends), so the
/// caller falls through to wrapping it instead of double-unwrapping
/// something `wrap_code` would never have produced.
fn unwrap_code(selected: &str) -> Option<String> {
    let leading = selected.chars().take_while(|&c| c == '`').count();
    let trailing = selected.chars().rev().take_while(|&c| c == '`').count();
    if leading == 0 || leading != trailing || selected.len() < leading + trailing {
        return None;
    }
    let inner = &selected[leading..selected.len() - trailing];
    if let Some(unpadded) = inner.strip_prefix(' ').and_then(|s| s.strip_suffix(' '))
        && (unpadded.starts_with('`') || unpadded.ends_with('`') || unpadded.is_empty())
    {
        // The space either side exists only because `wrap_code` had to pad
        // a leading/trailing backtick (or an empty selection); it is not
        // part of the original text.
        return Some(unpadded.to_string());
    }
    Some(inner.to_string())
}

/// The result of toggling `kind`'s markup around `selected` text: unwrapped
/// if `selected` is already wrapped in `kind`'s own markers, wrapped
/// otherwise (RFC-030). The one place this decision is made -- both
/// [`resolve_toggle_inline`] (a block's own content) and
/// `form::tables::resolve_toggle_inline_in_table_cell` (one table cell's
/// content, RFC-048 slice 2) call this, so a fix to how a format wraps or
/// unwraps (review, 2026-10-02 §3.1: the two had drifted into separate
/// copies) only has to happen once -- including task 047 Part C's
/// backtick-safe code span, below.
pub fn toggled_text(selected: &str, kind: InlineFormat, link_url: Option<&str>) -> String {
    if kind == InlineFormat::Code {
        return unwrap_code(selected).unwrap_or_else(|| wrap_code(selected));
    }

    let open_m = kind.open_marker();
    let close_m = kind.close_marker();

    if selected.starts_with(open_m)
        && selected.ends_with(close_m)
        && selected.len() >= open_m.len() + close_m.len()
    {
        // Unwrap: strip the markers.
        selected[open_m.len()..selected.len() - close_m.len()].to_string()
    } else {
        // Wrap: add markers.
        match kind {
            InlineFormat::Link => {
                let url = link_url.unwrap_or("");
                format!("[{selected}]({url})")
            }
            _ => format!("{open_m}{selected}{close_m}"),
        }
    }
}

/// Toggles inline markup around a UTF-16-offset selection within
/// `current_text` -- the block's own field as it currently stands in the
/// browser, not the document's last-committed text (task 048 D2). The
/// offsets are relative to `current_text`, since that is what the
/// browser's own `selectionStart`/`End` describe.
///
/// If the selected text is already wrapped in the same markers, the
/// markers are removed (unwrap). Otherwise they are added (wrap). The
/// result is **one** patch spanning the block's whole content range,
/// replacing it with `current_text` (the pending edit) plus the toggle --
/// so a click commits the field's own pending text and applies the
/// toggle together, never one without the other.
pub fn resolve_toggle_inline(
    block: &BlockNode,
    current_text: &str,
    kind: InlineFormat,
    utf16_start: usize,
    utf16_len: usize,
    link_url: Option<&str>,
) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    let content = block
        .content_range
        .ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "block has no content range".into(),
        })?;

    let byte_start = utf16_to_utf8_offset(current_text, utf16_start).ok_or_else(|| {
        FormEditError::InvalidEditPayload {
            reason: "invalid UTF-16 start offset".into(),
        }
    })?;
    let byte_end =
        utf16_to_utf8_offset(current_text, utf16_start + utf16_len).ok_or_else(|| {
            FormEditError::InvalidEditPayload {
                reason: "invalid UTF-16 end offset".into(),
            }
        })?;

    let selected = &current_text[byte_start..byte_end];
    let replacement = toggled_text(selected, kind, link_url);
    let new_text = format!(
        "{}{replacement}{}",
        &current_text[..byte_start],
        &current_text[byte_end..]
    );

    Ok((content, new_text, PatchOrigin::FormMode))
}
