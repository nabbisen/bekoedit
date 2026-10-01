#!/usr/bin/env python3
"""Print a release's notes: its own section of CHANGELOG.md, verbatim.

Usage: release-notes.py <version> <changelog-path> <repository> <tag>

docs/src/changelog-policy.md: the GitHub Release page carries the release's
CHANGELOG section as text, copied at the tag, so the page stands on its own
whatever later happens to CHANGELOG.md. The section's own heading is left out
(the release title already names the version); everything under it, from
### Highlights to the end of the section, is copied unchanged. A closing line
links to CHANGELOG.md at the same tag for the full history.

Task 043: while the Linux portability check's Arch entry still exempts
libxdo.so.3 (scripts/linux-portability-exemptions.tsv -- the same data
scripts/check-linux-portability-containers.sh reads, not a copy of it), the
page also carries a known-issue note after the CHANGELOG section and before
the closing line. Emptying that row removes the note, with no other edit.

Exits non-zero, naming the problem, if the version has no dated section or the
section is empty.
"""

import pathlib
import re
import sys

KNOWN_ISSUE_NOTE = (
    "### Known issue (Linux)\n"
    "This binary does not start on Arch Linux and Arch-based distributions, "
    "which ship `libxdo.so.4` instead of `libxdo.so.3`. Install with "
    "`cargo install bekoedit` instead."
)

EXEMPTIONS_FILE = pathlib.Path(__file__).parent / "linux-portability-exemptions.tsv"


def arch_libxdo_exemption_active(exemptions_path: pathlib.Path = EXEMPTIONS_FILE) -> bool:
    """Whether scripts/check-linux-portability-containers.sh's Arch row still
    exempts a missing library -- the single source of truth for the Linux
    known-issue note. Not any particular library name: once RFC-045 slice 3
    empties Arch's row, the note stops being printed in the same change,
    without this file changing too."""
    for line in exemptions_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        name, _, expect_missing = line.partition("\t")
        if name == "arch":
            return bool(expect_missing.strip())
    raise SystemExit(f"{exemptions_path}: no 'arch' row")


def section(changelog: str, version: str) -> str:
    heading = re.compile(rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}$", re.M)
    found = heading.search(changelog)
    if not found:
        raise SystemExit(f"CHANGELOG.md has no dated {version} heading")
    rest = changelog[found.end():]
    # The section ends at the next second-level heading, at the reference-link
    # definitions, or at the "Older releases:" footer, whichever comes first.
    end = re.search(r"^(## |\[[^\]]+\]: |Older releases:)", rest, re.M)
    body = (rest[: end.start()] if end else rest).strip("\n")
    if not body.strip():
        raise SystemExit(f"CHANGELOG.md {version} section is empty")
    return body


def notes_body(section_body: str, exemption_active: bool) -> str:
    """The CHANGELOG section, with the known-issue note appended when
    `exemption_active` says the Arch exemption is still live -- the one
    place that decides placement (after the section, before the footer)."""
    if exemption_active:
        return f"{section_body}\n\n{KNOWN_ISSUE_NOTE}"
    return section_body


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    version, path, repository, tag = sys.argv[1:]
    with open(path, encoding="utf-8") as handle:
        body = notes_body(section(handle.read(), version), arch_libxdo_exemption_active())
    link = f"https://github.com/{repository}/blob/{tag}/CHANGELOG.md"
    sys.stdout.write(f"{body}\n\n---\n\nFull history: [CHANGELOG.md]({link})\n")


if __name__ == "__main__":
    main()
