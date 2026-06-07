//! Resolution of semantic Form Mode edits into minimal `SourcePatch`
//! values (RFC-015 §8, RFC-018).
//!
//! Style preservation rules implemented here:
//! - Headings keep their original `#` marker run unless `SetHeadingLevel`
//!   explicitly changes it; setext headings reject level changes.
//! - Code blocks keep their fence marker, length, and indentation policy;
//!   the fence is lengthened only when the new code would collide with it.
//! - Task toggles patch exactly one byte inside the checkbox.
//! - Block deletion also removes the block's trailing blank lines so the
//!   document does not accumulate gaps.

use crate::block::{BlockKind, BlockNode, EditablePolicy, ListItemNode};
use crate::index::MarkdownIndex;
use crate::patch::{PatchOrigin, SourcePatch};
use crate::range::ByteRange;
use crate::trivia::LineEnding;

use super::{FormBlockEdit, FormEditCommand, FormEditError};

/// Validates `cmd` against `index` (which must describe `text`) and
/// produces the minimal source patch implementing the edit.
pub fn resolve_form_edit(
    text: &str,
    index: &MarkdownIndex,
    cmd: &FormEditCommand,
) -> Result<SourcePatch, FormEditError> {
    if cmd.base_revision != index.document_revision {
        return Err(FormEditError::DocumentRevisionMismatch {
            base: cmd.base_revision,
            current: index.document_revision,
        });
    }
    let block = index
        .resolve_block(&cmd.block_id)
        .ok_or(FormEditError::BlockNotFound)?;
    if let Some(fp) = cmd.client_block_fingerprint
        && fp != block.block_id.fingerprint
    {
        return Err(FormEditError::BlockFingerprintMismatch);
    }

    let (range, replacement, origin) = match &cmd.edit {
        FormBlockEdit::ReplacePlainText { text: new_text } => {
            resolve_replace_text(text, block, new_text)?
        }
        FormBlockEdit::SetHeadingLevel { level } => resolve_set_level(text, block, *level)?,
        FormBlockEdit::ToggleTaskChecked {
            item_ordinal,
            checked,
        } => resolve_toggle_task(text, block, *item_ordinal, *checked)?,
        FormBlockEdit::ReplaceListItemText {
            item_ordinal,
            text: new_text,
        } => resolve_item_text(block, *item_ordinal, new_text)?,
        FormBlockEdit::ReplaceCodeBlock { language, code } => {
            resolve_code(text, block, language.as_deref(), code)?
        }
        FormBlockEdit::ReplaceRawIsland { text: new_text } => resolve_island(block, new_text)?,
        FormBlockEdit::DeleteBlock => resolve_delete(text, block),
    };

    Ok(SourcePatch {
        base_revision: cmd.base_revision,
        range,
        replacement,
        origin,
    })
}

type Resolved = (ByteRange, String, PatchOrigin);

fn require_editable(block: &BlockNode) -> Result<(), FormEditError> {
    if block.editable_policy == EditablePolicy::FormEditable {
        Ok(())
    } else {
        Err(FormEditError::UnsupportedEditOperation {
            reason: "block is not form-editable".into(),
        })
    }
}

fn resolve_replace_text(
    text: &str,
    block: &BlockNode,
    new_text: &str,
) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    let content = block
        .content_range
        .ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "block has no editable text content".into(),
        })?;
    let replacement = match block.kind {
        BlockKind::Paragraph => new_text.to_string(),
        BlockKind::Heading => {
            if new_text.contains('\n') {
                return Err(FormEditError::InvalidEditPayload {
                    reason: "heading text must be a single line".into(),
                });
            }
            new_text.to_string()
        }
        BlockKind::Blockquote => {
            // Simple blockquotes are single-line; re-prefix any new lines.
            let nl = LineEnding::detect(text).as_str();
            new_text.replace('\n', &format!("{nl}> "))
        }
        _ => {
            return Err(FormEditError::UnsupportedEditOperation {
                reason:
                    "plain text replacement applies to paragraphs, headings, and simple blockquotes"
                        .into(),
            });
        }
    };
    Ok((content, replacement, PatchOrigin::FormMode))
}

fn resolve_set_level(text: &str, block: &BlockNode, level: u8) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    if block.kind != BlockKind::Heading {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a heading block".into(),
        });
    }
    if !(1..=6).contains(&level) {
        return Err(FormEditError::InvalidEditPayload {
            reason: "heading level must be between 1 and 6".into(),
        });
    }
    let start = block.source_range.start;
    let slice = &text[start..block.source_range.end];
    let first_line = slice.lines().next().unwrap_or("");
    let indent = first_line.len() - first_line.trim_start().len();
    let trimmed = &first_line[indent..];
    if !trimmed.starts_with('#') {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "setext headings cannot change level safely; edit in Text Mode".into(),
        });
    }
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    let range = ByteRange::new(start + indent, start + indent + hashes);
    Ok((range, "#".repeat(level as usize), PatchOrigin::FormMode))
}

fn find_item(block: &BlockNode, ordinal: u32) -> Result<&ListItemNode, FormEditError> {
    block
        .items
        .iter()
        .find(|it| it.ordinal == ordinal)
        .ok_or(FormEditError::ItemNotFound { ordinal })
}

fn resolve_toggle_task(
    text: &str,
    block: &BlockNode,
    ordinal: u32,
    checked: bool,
) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    if block.kind != BlockKind::TaskList {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a task list".into(),
        });
    }
    let item = find_item(block, ordinal)?;
    if item.task_checked.is_none() {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "list item has no checkbox".into(),
        });
    }
    // The checkbox `[x]` sits immediately before the content range.
    let line = &text[item.source_range.start..item.content_range.start];
    let bracket = line
        .rfind('[')
        .ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "checkbox marker not found".into(),
        })?;
    let pos = item.source_range.start + bracket + 1;
    let range = ByteRange::new(pos, pos + 1);
    let replacement = if checked { "x" } else { " " }.to_string();
    Ok((range, replacement, PatchOrigin::FormMode))
}

fn resolve_item_text(
    block: &BlockNode,
    ordinal: u32,
    new_text: &str,
) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    if !matches!(
        block.kind,
        BlockKind::BulletList | BlockKind::OrderedList | BlockKind::TaskList
    ) {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a list block".into(),
        });
    }
    if new_text.contains('\n') {
        return Err(FormEditError::InvalidEditPayload {
            reason: "list item text must be a single line in MVP".into(),
        });
    }
    let item = find_item(block, ordinal)?;
    Ok((
        item.content_range,
        new_text.to_string(),
        PatchOrigin::FormMode,
    ))
}

fn resolve_code(
    text: &str,
    block: &BlockNode,
    language: Option<&str>,
    code: &str,
) -> Result<Resolved, FormEditError> {
    require_editable(block)?;
    if block.kind != BlockKind::FencedCode {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "not a fenced code block".into(),
        });
    }
    let style =
        block
            .trivia
            .code_fence_style
            .ok_or_else(|| FormEditError::UnsupportedEditOperation {
                reason: "missing fence style".into(),
            })?;
    // Lengthen the fence only if the new code collides with it.
    let longest_run = code
        .lines()
        .map(|l| {
            let t = l.trim_start();
            t.chars().take_while(|c| *c == style.marker).count()
        })
        .max()
        .unwrap_or(0);
    let fence_len = style.length.max(longest_run + 1).max(3);
    let fence: String = std::iter::repeat_n(style.marker, fence_len).collect();
    let nl = LineEnding::detect(text).as_str();
    let lang = language.unwrap_or("").trim();
    let mut replacement = format!("{fence}{lang}{nl}");
    replacement.push_str(code);
    if !code.is_empty() && !code.ends_with('\n') {
        replacement.push_str(nl);
    }
    replacement.push_str(&fence);
    Ok((block.source_range, replacement, PatchOrigin::FormMode))
}

fn resolve_island(block: &BlockNode, new_text: &str) -> Result<Resolved, FormEditError> {
    if block.editable_policy != EditablePolicy::RawIslandOnly {
        return Err(FormEditError::UnsupportedEditOperation {
            reason: "block is not a raw island".into(),
        });
    }
    Ok((
        block.source_range,
        new_text.to_string(),
        PatchOrigin::RawIsland,
    ))
}

fn resolve_delete(text: &str, block: &BlockNode) -> Resolved {
    // Remove the block plus its trailing newline and following blank lines.
    let mut end = block.source_range.end;
    let bytes = text.as_bytes();
    if end < bytes.len() && bytes[end] == b'\r' && end + 1 < bytes.len() && bytes[end + 1] == b'\n'
    {
        end += 2;
    } else if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    loop {
        let line_end = text[end..]
            .find('\n')
            .map(|p| end + p + 1)
            .unwrap_or(text.len());
        if end < text.len() && text[end..line_end].trim().is_empty() && line_end > end {
            end = line_end;
        } else {
            break;
        }
    }
    (
        ByteRange::new(block.source_range.start, end),
        String::new(),
        PatchOrigin::FormMode,
    )
}
