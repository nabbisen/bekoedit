// Link-click decisions (task 032) -- bekoedit-core.
//
// The decision is the one place that says whether a click may reach the OS
// opener, so every outcome is pinned, including each way of leaving the
// workspace. Each test is proven by a mutation named in the task 032 review
// request.

use std::path::PathBuf;

use crate::link_click::{LinkAction, LinkRefusal, decide_link_click};

/// A workspace `root/` holding `a.md`, `sub/b.md`, `sub/notes.txt` and
/// `sub/folder/` and `sub/dir.md/`, beside `outside/secret.md`, which is not in the workspace.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let root = base.join("root");
    let outside = base.join("outside");
    std::fs::create_dir_all(root.join("sub/folder")).unwrap();
    // A directory that is named like a document.
    std::fs::create_dir_all(root.join("sub/dir.md")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(root.join("a.md"), "# a\n").unwrap();
    std::fs::write(root.join("sub/b.md"), "# b\n").unwrap();
    std::fs::write(root.join("sub/notes.txt"), "notes\n").unwrap();
    std::fs::write(outside.join("secret.md"), "# secret\n").unwrap();
    Fixture {
        _dir: dir,
        root,
        outside,
    }
}

fn click(fixture: &Fixture, href: &str) -> LinkAction {
    let document = fixture.root.join("sub/b.md");
    decide_link_click(href, Some(&document), Some(&fixture.root))
}

fn refused(fixture: &Fixture, href: &str) -> LinkRefusal {
    match click(fixture, href) {
        LinkAction::Refuse(reason) => reason,
        other => panic!("{href:?} should be refused, got {other:?}"),
    }
}

fn opens_externally(action: &LinkAction) -> Option<&str> {
    match action {
        LinkAction::OpenExternal(url) => Some(url.as_str()),
        _ => None,
    }
}

#[test]
fn http_https_and_mailto_open_externally() {
    let fixture = fixture();
    for href in [
        "http://example.com/x",
        "https://example.com/x?y=1#z",
        "mailto:someone@example.com",
    ] {
        assert_eq!(
            opens_externally(&click(&fixture, href)),
            Some(href),
            "{href}"
        );
    }
}

#[test]
fn an_external_url_is_the_normalised_destination_that_was_classified() {
    let fixture = fixture();
    // A browser strips these before it reads the scheme, so the opener is
    // handed the string that was checked, not the raw one.
    assert_eq!(
        opens_externally(&click(&fixture, "  \u{1}HTTPS://example.com/a\tb")),
        Some("HTTPS://example.com/ab")
    );
}

#[test]
fn no_other_scheme_reaches_the_opener() {
    let fixture = fixture();
    for href in [
        "javascript:alert(1)",
        " \tJaVaScRiPt:alert(1)",
        "file:///etc/passwd",
        "data:text/html,x",
        "vbscript:x",
        "ftp://example.com/x",
        "ssh://example.com",
        "c:/Windows/notepad.exe",
        "tel:123",
    ] {
        assert_eq!(
            refused(&fixture, href),
            LinkRefusal::UnsupportedScheme,
            "{href}"
        );
    }
}

#[test]
fn a_fragment_stays_in_the_page() {
    let fixture = fixture();
    assert_eq!(
        click(&fixture, "#section-2"),
        LinkAction::InPage("section-2".to_string())
    );
    assert_eq!(click(&fixture, "#"), LinkAction::InPage(String::new()));
}

#[test]
fn a_relative_path_inside_the_workspace_opens_in_bekoedit() {
    let fixture = fixture();
    for (href, expected) in [
        ("../a.md", "a.md"),
        ("./b.md", "sub/b.md"),
        ("b.md", "sub/b.md"),
        ("b.md#top", "sub/b.md"),
        ("b.md?x=1", "sub/b.md"),
        ("../sub/../a.md", "a.md"),
        ("folder/../b.md", "sub/b.md"),
    ] {
        assert_eq!(
            click(&fixture, href),
            LinkAction::OpenDocument(PathBuf::from(expected)),
            "{href}"
        );
    }
}

#[test]
fn a_percent_encoded_path_is_decoded_before_it_is_resolved() {
    let fixture = fixture();
    std::fs::write(fixture.root.join("sub/my file é.md"), "x\n").unwrap();
    assert_eq!(
        click(&fixture, "my%20file%20%C3%A9.md"),
        LinkAction::OpenDocument(PathBuf::from("sub/my file é.md"))
    );
    // Decoding must not smuggle a separator past the checks.
    assert_eq!(
        refused(&fixture, "..%2F..%2Foutside%2Fsecret.md"),
        LinkRefusal::OutsideWorkspace
    );
    assert_eq!(
        refused(&fixture, "%2F%2Fhost%2Fshare"),
        LinkRefusal::NetworkPath
    );
    assert_eq!(
        refused(&fixture, "%2Fetc%2Fpasswd"),
        LinkRefusal::AbsolutePath
    );
    assert_eq!(refused(&fixture, "%FF.md"), LinkRefusal::InvalidPath);
    assert_eq!(refused(&fixture, "a%00.md"), LinkRefusal::InvalidPath);
}

#[test]
fn a_path_that_climbs_out_of_the_workspace_is_refused() {
    let fixture = fixture();
    // It exists, so only the containment check can refuse it.
    assert!(fixture.outside.join("secret.md").is_file());
    for href in [
        "../../outside/secret.md",
        "../../outside/missing.md",
        "../../../../../../../../etc/passwd",
        "folder/../../../outside/secret.md",
    ] {
        assert_eq!(
            refused(&fixture, href),
            LinkRefusal::OutsideWorkspace,
            "{href}"
        );
    }
}

#[test]
fn an_absolute_path_is_refused_even_inside_the_workspace() {
    let fixture = fixture();
    let inside = format!("{}", fixture.root.join("a.md").display());
    for href in [
        "/etc/passwd",
        "/a.md",
        "\\Windows\\win.ini",
        inside.as_str(),
    ] {
        let reason = refused(&fixture, href);
        assert!(
            matches!(
                reason,
                LinkRefusal::AbsolutePath | LinkRefusal::UnsupportedScheme
            ),
            "{href}: {reason:?}"
        );
    }
    assert_eq!(refused(&fixture, "/a.md"), LinkRefusal::AbsolutePath);
    assert_eq!(refused(&fixture, "\\a.md"), LinkRefusal::AbsolutePath);
}

#[test]
fn a_network_path_reference_is_refused_in_every_spelling() {
    let fixture = fixture();
    for href in [
        "//host/share/a.md",
        "\\\\host\\share\\a.md",
        "/\\host/share",
        "\\/host/share",
        " \t//host/share",
    ] {
        assert_eq!(refused(&fixture, href), LinkRefusal::NetworkPath, "{href}");
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_workspace_is_refused() {
    let fixture = fixture();
    std::os::unix::fs::symlink(
        fixture.outside.join("secret.md"),
        fixture.root.join("sub/escape.md"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&fixture.outside, fixture.root.join("sub/outdir")).unwrap();
    assert_eq!(
        refused(&fixture, "escape.md"),
        LinkRefusal::OutsideWorkspace
    );
    assert_eq!(
        refused(&fixture, "outdir/secret.md"),
        LinkRefusal::OutsideWorkspace
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_that_stays_inside_the_workspace_opens_its_target() {
    let fixture = fixture();
    std::os::unix::fs::symlink(fixture.root.join("a.md"), fixture.root.join("sub/alias.md"))
        .unwrap();
    assert_eq!(
        click(&fixture, "alias.md"),
        LinkAction::OpenDocument(PathBuf::from("a.md"))
    );
}

#[test]
fn only_an_existing_markdown_file_opens() {
    let fixture = fixture();
    assert_eq!(refused(&fixture, "missing.md"), LinkRefusal::NotFound);
    assert_eq!(refused(&fixture, "notes.txt"), LinkRefusal::NotADocument);
    assert_eq!(refused(&fixture, "folder"), LinkRefusal::NotADocument);
    assert_eq!(refused(&fixture, "dir.md"), LinkRefusal::NotADocument);
    assert_eq!(refused(&fixture, "."), LinkRefusal::NotADocument);
}

#[test]
fn a_link_with_no_destination_is_refused_not_silently_dropped() {
    let fixture = fixture();
    assert_eq!(refused(&fixture, ""), LinkRefusal::NoTarget);
    assert_eq!(refused(&fixture, "   "), LinkRefusal::NoTarget);
    assert_eq!(refused(&fixture, "?only-a-query"), LinkRefusal::NoTarget);
}

#[test]
fn a_relative_path_needs_a_document_and_a_workspace_to_resolve_against() {
    let fixture = fixture();
    let document = fixture.root.join("sub/b.md");
    let no_document = decide_link_click("a.md", None, Some(&fixture.root));
    let no_workspace = decide_link_click("a.md", Some(&document), None);
    let neither = decide_link_click("a.md", None, None);
    for action in [no_document, no_workspace, neither] {
        assert_eq!(action, LinkAction::Refuse(LinkRefusal::NoBase));
    }
    // A missing base never stops the two cases that need none.
    assert!(matches!(
        decide_link_click("https://example.com", None, None),
        LinkAction::OpenExternal(_)
    ));
    assert_eq!(
        decide_link_click("#x", None, None),
        LinkAction::InPage("x".to_string())
    );
}

#[test]
fn a_document_outside_the_workspace_cannot_reach_into_it_by_accident() {
    let fixture = fixture();
    // An untitled or foreign document resolves against its own directory,
    // which is outside the root, so a plain sibling name is refused.
    let foreign = fixture.outside.join("secret.md");
    let action = decide_link_click("secret.md", Some(&foreign), Some(&fixture.root));
    assert_eq!(action, LinkAction::Refuse(LinkRefusal::OutsideWorkspace));
}
