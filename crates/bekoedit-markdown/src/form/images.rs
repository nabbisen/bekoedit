//! Image block resolution for Form Mode (RFC-028).

use crate::block::BlockNode;
use crate::form::FormEditError;
use crate::patch::PatchOrigin;
use crate::range::ByteRange;

type Resolved = (ByteRange, String, PatchOrigin);
pub fn resolve_replace_image(
    text: &str,
    block: &BlockNode,
    alt: &str,
    src: &str,
) -> Result<Resolved, crate::form::FormEditError> {
    // Accept any block — images may appear inside paragraphs or as standalone blocks. — images may appear inside paragraphs or as standalone blocks.
    let source = text
        .get(block.source_range.start..block.source_range.end)
        .ok_or_else(|| FormEditError::UnsupportedEditOperation {
            reason: "source range out of bounds".into(),
        })?;
    // Find the image markdown `![...](...)` and replace alt + src.
    // Minimal regex-free parser: locate ![...](... ) pattern.
    let new_source = rewrite_image_source(source, alt, src);
    Ok((block.source_range, new_source, PatchOrigin::FormMode))
}

/// Rewrites `![alt](src)` → `![new_alt](new_src)` preserving any title.
fn rewrite_image_source(source: &str, new_alt: &str, new_src: &str) -> String {
    if let Some(bang) = source.find("![") {
        let after_bang = &source[bang + 2..];
        if let Some(close_bracket) = after_bang.find("](") {
            let after_open_paren = &after_bang[close_bracket + 2..];
            // Find the closing paren, optionally skipping a title.
            let close_paren = after_open_paren
                .rfind(')')
                .unwrap_or(after_open_paren.len());
            let href_part = &after_open_paren[..close_paren];
            // Preserve title if present: `src "title"` or `src 'title'`.
            let title = if let Some(t_start) = href_part.find(['"', '\'']) {
                format!(" {}", &href_part[t_start..])
            } else {
                String::new()
            };
            return format!(
                "{}![{}]({}{}){}",
                &source[..bang],
                new_alt,
                new_src,
                title,
                &source[bang + 2 + close_bracket + 2 + close_paren + 1..]
            );
        }
    }
    // Fallback: return a fresh image tag.
    format!("![{}]({})", new_alt, new_src)
}
