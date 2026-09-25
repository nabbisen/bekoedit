// Destination classification (task 032), the reader shared by Preview's
// render policy and its click policy.

use crate::destination::{DestinationKind, classify_destination, normalized};

#[test]
fn each_kind_is_recognised() {
    use DestinationKind::*;
    for (destination, expected) in [
        ("http://example.com", Web),
        ("https://example.com/a?b#c", Web),
        ("HTTPS://EXAMPLE.COM", Web),
        ("mailto:a@example.com", Mail),
        ("MailTo:a@example.com", Mail),
        ("#section", Fragment),
        ("#", Fragment),
        ("other.md", Path),
        ("../up/other.md", Path),
        ("/rooted.md", Path),
        ("", Path),
        ("//host/share", NetworkPath),
        ("\\\\host\\share", NetworkPath),
        ("/\\host", NetworkPath),
        ("\\/host", NetworkPath),
        ("javascript:alert(1)", Other),
        ("file:///etc/passwd", Other),
        ("data:text/html,x", Other),
        ("c:/a.exe", Other),
        ("tel:1", Other),
    ] {
        assert_eq!(
            classify_destination(destination),
            expected,
            "{destination:?}"
        );
    }
}

#[test]
fn a_browsers_scheme_normalisation_is_applied_first() {
    use DestinationKind::*;
    // Leading controls and spaces are stripped, tab and newlines removed
    // anywhere, exactly as the URL parser does before it looks for a scheme.
    for (destination, expected) in [
        ("  \u{1}https://example.com", Web),
        ("\thttps://example.com", Web),
        ("ht\ttps://example.com", Web),
        ("java\nscript:alert(1)", Other),
        (" \u{0}javascript:alert(1)", Other),
        ("  #frag", Fragment),
        (" \t//host/share", NetworkPath),
    ] {
        assert_eq!(
            classify_destination(destination),
            expected,
            "{destination:?}"
        );
    }
    assert_eq!(normalized("  \u{1}ht\ttps://x\r\n"), "https://x");
}

#[test]
fn a_colon_after_a_slash_or_dot_is_a_path_not_a_scheme() {
    use DestinationKind::*;
    for destination in ["a/b:c", "./x:y", "9lives:x", "a b:c", ":x", "%68ttp://x"] {
        assert_eq!(classify_destination(destination), Path, "{destination:?}");
    }
}
