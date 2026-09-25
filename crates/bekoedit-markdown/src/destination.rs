//! Link-destination classification, shared by Preview's render policy
//! (task 030) and its click policy (task 032), so the two cannot drift: what
//! the renderer lets through as an `href` and what a click is allowed to do
//! with it are read by the same code.
//!
//! A destination is read the way a browser's URL parser reads it. It is
//! never opened, fetched or resolved here.

/// The destination as a browser's URL parser sees it before looking for a
/// scheme: leading C0 control characters and spaces are stripped, and ASCII
/// tab and newlines are removed everywhere. (The destination arrives here
/// already entity-decoded, so `&#106;avascript:` is `javascript:`.)
pub fn normalized(destination: &str) -> String {
    destination
        .trim_start_matches(|c: char| c.is_ascii_control() || c == ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// The lowercase scheme, or `None` for a relative path or a `#fragment`.
pub(crate) fn scheme(normalized: &str) -> Option<String> {
    let name = &normalized[..normalized.find(':')?];
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| name.to_ascii_lowercase())
}

/// A destination with no scheme that begins with two slashes of either kind
/// (`//`, `\\`, `/\` or `\/`, after normalisation) is a network-path
/// reference, not a relative path: a browser reads it as a host, and on Windows
/// `\\host\share` is a UNC path, which opening makes the OS contact over SMB.
pub(crate) fn is_network_path(normalized: &str) -> bool {
    let mut chars = normalized.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    )
}

/// What kind of thing a link destination names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationKind {
    /// `http:` or `https:`.
    Web,
    /// `mailto:`.
    Mail,
    /// Begins with `#`: a place inside the current document.
    Fragment,
    /// No scheme, not a fragment, not a network path: a path to a file.
    /// The empty destination is this kind, with an empty path.
    Path,
    /// See `is_network_path`.
    NetworkPath,
    /// Any other scheme: `javascript:`, `file:`, `data:`, `vbscript:`, a
    /// Windows drive such as `c:`, an application's own scheme.
    Other,
}

/// Classifies a raw destination, as it appears in a Markdown link or in an
/// `href` attribute.
pub fn classify_destination(destination: &str) -> DestinationKind {
    let normalized = normalized(destination);
    match scheme(&normalized).as_deref() {
        Some("http" | "https") => DestinationKind::Web,
        Some("mailto") => DestinationKind::Mail,
        Some(_) => DestinationKind::Other,
        None if is_network_path(&normalized) => DestinationKind::NetworkPath,
        None if normalized.starts_with('#') => DestinationKind::Fragment,
        None => DestinationKind::Path,
    }
}
