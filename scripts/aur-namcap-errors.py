#!/usr/bin/env python3
"""The namcap lines that fail the AUR proof (RFC-045 slice 3b).

Usage: namcap-output | scripts/aur-namcap-errors.py

Reads namcap's output on stdin and prints every error line (`<name> E: ...`),
and exits 1 if there was one, 0 if there was none. Warnings (`W:`) never fail.

ONE error is not counted, by name and by shape: the debug package's build-id
symlinks. makepkg on Arch's default configuration builds a `<pkgname>-debug`
package whose `usr/lib/debug/.build-id/xx/yyyy` symlink points (as
`../../../../bin/<binary>`) into the MAIN package, so namcap, which checks the
debug package on its own, reports

    <pkgname>-debug E: Symlink (usr/lib/debug/.build-id/xx/yyyy) points to non-existing ../../../../bin/<binary>

for a package that is correct. The line is excused only when the package name ends
in `-debug`, the symlink is under `usr/lib/debug/.build-id/`, and the target
starts with `../../../../` (up out of `usr/lib/debug/.build-id/xx` to `usr/`).
Any other error, in the debug package or in any other, still fails.
"""

import re
import sys

ERROR = re.compile(r"^(?P<name>\S+)(?: \((?P<file>[^)]*)\))? E: (?P<message>.*)$")
BUILD_ID_SYMLINK = re.compile(
    r"^Symlink \(usr/lib/debug/\.build-id/[0-9a-f]{2}/[0-9a-f]+(?:\.debug)?\) points to non-existing \.\./\.\./\.\./\.\./\S+$"
)


def excused(line: str) -> bool:
    found = ERROR.match(line)
    return bool(found and found.group("name").endswith("-debug") and BUILD_ID_SYMLINK.match(found.group("message")))


def errors(text: str) -> list[str]:
    return [line for line in text.splitlines() if " E: " in line and not excused(line)]


def main() -> int:
    found = errors(sys.stdin.read())
    for line in found:
        print(line)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
