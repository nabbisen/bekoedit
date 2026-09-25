//! What a click on a link in rendered Markdown may do (task 032).
//!
//! The click handler in the app forwards the clicked `href` here and does
//! exactly what comes back. **Only `http:`, `https:` and `mailto:` ever
//! yield `OpenExternal`**, and `ExternalUrl` can be built nowhere else, so
//! nothing but a decided URL can reach the OS opener. A relative path is
//! resolved against the current document's directory and must land on an
//! existing Markdown file inside the workspace, after canonicalisation
//! (which follows symlinks, so a link out of the workspace is refused).
//! It is never handed to the OS: before this task it was, as a path
//! relative to the process's working directory.

use std::path::{Component, Path, PathBuf};

use bekoedit_fs::paths::is_markdown_path;
use bekoedit_markdown::{DestinationKind, classify_destination, normalize_destination};

/// A URL that may be handed to the OS opener. Only `decide_link_click` makes
/// one, and only from an `http:`, `https:` or `mailto:` destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrl(String);

impl ExternalUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a link was not followed. The app shows each as a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRefusal {
    /// The link has no destination.
    NoTarget,
    /// A scheme other than `http:`, `https:` and `mailto:`.
    UnsupportedScheme,
    /// `//host/x` or `\\host\x`: names another machine.
    NetworkPath,
    /// A path from the filesystem root, or with a drive or UNC prefix.
    AbsolutePath,
    /// The destination is not a valid path (bad percent-encoding, NUL).
    InvalidPath,
    /// A relative path with no open document or no open workspace to
    /// resolve it against.
    NoBase,
    /// Resolves outside the workspace root, lexically or through a symlink.
    OutsideWorkspace,
    /// Resolves inside the workspace to nothing that exists.
    NotFound,
    /// Exists, but is not a Markdown file.
    NotADocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    /// Hand to the system browser or mail client.
    OpenExternal(ExternalUrl),
    /// A `#fragment`: stays in the page. The text after `#`.
    InPage(String),
    /// A Markdown file inside the workspace, as a workspace-relative path
    /// (the form `AppState::open_document` takes).
    OpenDocument(PathBuf),
    Refuse(LinkRefusal),
}

/// Decides what a click on `href` does. `document` is the open document's
/// path and `workspace_root` the workspace's root.
pub fn decide_link_click(
    href: &str,
    document: Option<&Path>,
    workspace_root: Option<&Path>,
) -> LinkAction {
    match classify_destination(href) {
        DestinationKind::Web | DestinationKind::Mail => {
            LinkAction::OpenExternal(ExternalUrl(normalize_destination(href)))
        }
        DestinationKind::Fragment => {
            let normalized = normalize_destination(href);
            LinkAction::InPage(normalized[1..].to_string())
        }
        DestinationKind::NetworkPath => LinkAction::Refuse(LinkRefusal::NetworkPath),
        DestinationKind::Other => LinkAction::Refuse(LinkRefusal::UnsupportedScheme),
        DestinationKind::Path => match resolve_document(href, document, workspace_root) {
            Ok(relative) => LinkAction::OpenDocument(relative),
            Err(refusal) => LinkAction::Refuse(refusal),
        },
    }
}

fn resolve_document(
    href: &str,
    document: Option<&Path>,
    workspace_root: Option<&Path>,
) -> Result<PathBuf, LinkRefusal> {
    let normalized = normalize_destination(href);
    let path_part = normalized.split(['#', '?']).next().unwrap_or("");
    if path_part.is_empty() {
        return Err(LinkRefusal::NoTarget);
    }
    let decoded = percent_decode(path_part).ok_or(LinkRefusal::InvalidPath)?;
    if decoded.contains('\0') {
        return Err(LinkRefusal::InvalidPath);
    }
    // After decoding, so `%2F%2Fhost` is caught like `//host`.
    let mut leading = decoded.chars();
    match (leading.next(), leading.next()) {
        (Some('/' | '\\'), Some('/' | '\\')) => return Err(LinkRefusal::NetworkPath),
        (Some('/' | '\\'), _) => return Err(LinkRefusal::AbsolutePath),
        _ => {}
    }
    let requested = Path::new(&decoded);
    if requested.is_absolute()
        || requested
            .components()
            .any(|part| matches!(part, Component::RootDir | Component::Prefix(_)))
    {
        return Err(LinkRefusal::AbsolutePath);
    }

    let (Some(document), Some(root)) = (document, workspace_root) else {
        return Err(LinkRefusal::NoBase);
    };
    let root = root.canonicalize().map_err(|_| LinkRefusal::NoBase)?;
    let directory = document.parent().ok_or(LinkRefusal::NoBase)?;
    let mut target = directory.canonicalize().map_err(|_| LinkRefusal::NoBase)?;
    // Lexical resolution first, so `../../elsewhere` is reported as leaving
    // the workspace whether or not it exists.
    for part in requested.components() {
        match part {
            Component::Normal(name) => target.push(name),
            Component::ParentDir => {
                target.pop();
            }
            _ => {}
        }
    }
    if !target.starts_with(&root) {
        return Err(LinkRefusal::OutsideWorkspace);
    }
    // Then the filesystem's answer, which follows symlinks.
    let canonical = target.canonicalize().map_err(|_| LinkRefusal::NotFound)?;
    if !canonical.starts_with(&root) {
        return Err(LinkRefusal::OutsideWorkspace);
    }
    if !canonical.is_file() || !is_markdown_path(&canonical) {
        return Err(LinkRefusal::NotADocument);
    }
    canonical
        .strip_prefix(&root)
        .map(Path::to_path_buf)
        .map_err(|_| LinkRefusal::OutsideWorkspace)
}

/// Decodes `%XX` escapes. A `%` not followed by two hex digits stays literal,
/// as in a browser. `None` if the result is not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| hex_pair(bytes[i + 1], bytes[i + 2]))
            .flatten();
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    let digit = |c: u8| (c as char).to_digit(16);
    Some((digit(high)? * 16 + digit(low)?) as u8)
}
