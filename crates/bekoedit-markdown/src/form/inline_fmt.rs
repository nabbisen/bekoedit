//! Inline formatting toggle resolution (RFC-030).
//!
//! Converts `ToggleInline` commands with UTF-16 selection offsets into
//! minimal source patches that wrap or unwrap `**bold**`, `_italic_`,
//! `` `code` ``, and `[link](url)` around the selected text.

use crate::block::BlockNode;
use crate::form::{FormEditError, InlineFormat};
use crate::patch::PatchOrigin;
use crate::range::{ByteRange, utf16_to_utf8_offset};

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

/// Toggles inline markup around a UTF-16-offset selection within the
/// block's content (RFC-030).
///
/// If the selected text is already wrapped in the same markers, the
/// markers are removed (unwrap). Otherwise they are added (wrap).
pub fn resolve_toggle_inline(
    text: &str,
    block: &BlockNode,
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
    let content_text = &text[content.start..content.end];

    let byte_start = utf16_to_utf8_offset(content_text, utf16_start).ok_or_else(|| {
        FormEditError::InvalidEditPayload {
            reason: "invalid UTF-16 start offset".into(),
        }
    })?;
    let byte_end_local =
        utf16_to_utf8_offset(content_text, utf16_start + utf16_len).ok_or_else(|| {
            FormEditError::InvalidEditPayload {
                reason: "invalid UTF-16 end offset".into(),
            }
        })?;

    let selected = &content_text[byte_start..byte_end_local];
    let open_m = kind.open_marker();
    let close_m = kind.close_marker();

    let replacement = if selected.starts_with(open_m)
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
    };

    let abs_start = content.start + byte_start;
    let abs_end = content.start + byte_end_local;
    Ok((
        ByteRange::new(abs_start, abs_end),
        replacement,
        PatchOrigin::FormMode,
    ))
}
