#!/usr/bin/env python3
"""Pick the built package file that is exactly PKGNAME's (RFC-045 slice 3b).

Usage: scripts/aur-pick-package.py PKGNAME FILE...

`makepkg` on Arch's default configuration also builds a `<pkgname>-debug` package
(its default `OPTIONS` has `debug`; some machines' configurations turn it off, so
a local build may not show it). The proof installs, starts and removes exactly the
package under proof, never the debug one: installing every built file leaves the
first package's `-debug` behind, owning the same debug file as the next package's,
and the second install then stops on conflicting files.

Prints the one FILE whose package name is exactly PKGNAME. Refuses, naming what
was built, when there is none, or more than one. Only file names are read.

A package file is `<pkgname>-<pkgver>-<pkgrel>-<arch>.pkg.tar.<ext>`; the last
three dash-separated fields are version, release and architecture, and pkgname is
everything before them (it may itself contain dashes, as `bekoedit-bin` does).
"""

import pathlib
import re
import sys

PACKAGE_FILE = re.compile(r"^(?P<name>.+)-(?P<ver>[^-]+)-(?P<rel>[^-]+)-(?P<arch>[^-]+)\.pkg\.tar(\.[A-Za-z0-9]+)?$")


def package_name(file_name: str) -> str | None:
    found = PACKAGE_FILE.match(file_name)
    return found.group("name") if found else None


def pick(name: str, files: list[str]) -> str:
    """The one file that is `name`'s package; ValueError, naming the built files, if not."""
    built = [pathlib.PurePath(f).name for f in files]
    matching = [f for f in files if package_name(pathlib.PurePath(f).name) == name]
    if len(matching) == 1:
        return matching[0]
    reason = "no built file is" if not matching else f"{len(matching)} built files are"
    raise ValueError(f"{reason} package '{name}' (built: {', '.join(built) or 'nothing'})")


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        print(pick(argv[1], argv[2:]))
    except ValueError as refusal:
        print(f"aur-pick-package: refused: {refusal}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
