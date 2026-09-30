#!/usr/bin/env python3
"""Print a release's notes: its own section of CHANGELOG.md, verbatim.

Usage: release-notes.py <version> <changelog-path> <repository> <tag>

docs/src/changelog-policy.md: the GitHub Release page carries the release's
CHANGELOG section as text, copied at the tag, so the page stands on its own
whatever later happens to CHANGELOG.md. The section's own heading is left out
(the release title already names the version); everything under it, from
### Highlights to the end of the section, is copied unchanged. A closing line
links to CHANGELOG.md at the same tag for the full history.

Exits non-zero, naming the problem, if the version has no dated section or the
section is empty.
"""

import re
import sys


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


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit(__doc__)
    version, path, repository, tag = sys.argv[1:]
    with open(path, encoding="utf-8") as handle:
        body = section(handle.read(), version)
    link = f"https://github.com/{repository}/blob/{tag}/CHANGELOG.md"
    sys.stdout.write(f"{body}\n\n---\n\nFull history: [CHANGELOG.md]({link})\n")


if __name__ == "__main__":
    main()
