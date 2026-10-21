#!/usr/bin/env python3
"""Render a PKGBUILD template into the PKGBUILD pushed to the AUR (RFC-045 slice 3b).

Usage:
  scripts/aur-render.py VERSION TEMPLATE OUTPUT_DIR FILE...

FILE... are the files the template's `source` entries point to, in source order
(the tag's tarball; or the Arch archive, the desktop file and the icon). Writes
OUTPUT_DIR/PKGBUILD, then OUTPUT_DIR/.SRCINFO from `makepkg --printsrcinfo` on
that file. `.SRCINFO` is never written by hand, and makepkg refuses to run as
root, so neither does this: run it as an ordinary user.

What it does to the template:
  - `pkgver` must already be VERSION, and `pkgrel` must be 1; both are kept;
  - every `SKIP` in `sha256sums` is replaced by the sha256 of the matching
    FILE, in source order;
  - every block between a `# >>> template-only` line and a `# <<< template-only`
    line is dropped, markers included: those comments describe the template,
    and would be false in the AUR copy.

It refuses, naming the reason and writing nothing, when: the template's pkgver is
not VERSION, or its pkgrel is not 1; the number of FILEs differs from the number
of `source` entries or of `sha256sums`; a `sha256sums` entry is not `SKIP` or any
`SKIP` would remain; or the template-only markers are unbalanced.
"""

import hashlib
import pathlib
import re
import shutil
import subprocess
import sys

OPEN = "# >>> template-only"
CLOSE = "# <<< template-only"
VERSION_RE = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")


class Refusal(Exception):
    """A template or input the renderer will not render."""


def drop_template_only(text: str) -> str:
    out, inside = [], False
    for number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped == OPEN:
            if inside:
                raise Refusal(f"line {number}: '{OPEN}' inside an open template-only block")
            inside = True
        elif stripped == CLOSE:
            if not inside:
                raise Refusal(f"line {number}: '{CLOSE}' without an opening marker")
            inside = False
        elif not inside:
            out.append(line)
    if inside:
        raise Refusal(f"'{OPEN}' is never closed")
    return "\n".join(out) + "\n"


def variable(text: str, name: str) -> str:
    found = re.search(rf"^{name}=(.*)$", text, re.M)
    if not found:
        raise Refusal(f"the template has no {name}=")
    return found.group(1).strip().strip("'\"")


def array_entries(body: str) -> list[str]:
    """The quoted entries of a bash array body, in order."""
    return [single or double for single, double in re.findall(r"'([^']*)'|\"([^\"]*)\"", body)]


def sums_line(text: str) -> re.Match:
    found = re.search(r"^sha256sums=\(([^)]*)\)[ \t]*$", text, re.M)
    if not found:
        raise Refusal("the template has no single-line sha256sums=( ... )")
    return found


def source_count(text: str) -> int:
    found = re.search(r"^source=\(([^)]*)\)", text, re.M | re.S)
    if not found:
        raise Refusal("the template has no source=( ... )")
    return len(array_entries(found.group(1)))


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def render(version: str, template: str, files: list[pathlib.Path]) -> str:
    """The PKGBUILD text. Pure apart from reading `files`."""
    if not VERSION_RE.fullmatch(version):
        raise Refusal(f"{version!r} is not bare SemVer")
    pkgver, pkgrel = variable(template, "pkgver"), variable(template, "pkgrel")
    if pkgver != version:
        raise Refusal(f"the template's pkgver is {pkgver}, not {version}")
    if pkgrel != "1":
        raise Refusal(f"the template's pkgrel is {pkgrel}, not 1")

    sources = source_count(template)
    line = sums_line(template)
    sums = array_entries(line.group(1))
    if len(sums) != sources:
        raise Refusal(f"the template has {sources} source entries but {len(sums)} sha256sums")
    if len(files) != len(sums):
        raise Refusal(f"{len(files)} files were given for {len(sums)} sha256sums entries")
    not_skip = [entry for entry in sums if entry != "SKIP"]
    if not_skip:
        raise Refusal(f"the template's sha256sums holds a real sum, not SKIP: {not_skip[0]}")
    for path in files:
        if not path.is_file():
            raise Refusal(f"{path} is not a file")

    rendered_sums = " ".join(f"'{sha256(path)}'" for path in files)
    rendered = template[: line.start()] + f"sha256sums=({rendered_sums})" + template[line.end():]
    rendered = drop_template_only(rendered)
    if re.search(r"\bSKIP\b", rendered):
        raise Refusal("a SKIP remains in the rendered PKGBUILD")
    return rendered


def main(argv: list[str]) -> int:
    if len(argv) < 5:
        print(__doc__, file=sys.stderr)
        return 2
    version, template_path, output_dir = argv[1], pathlib.Path(argv[2]), pathlib.Path(argv[3])
    files = [pathlib.Path(name) for name in argv[4:]]
    try:
        rendered = render(version, template_path.read_text(encoding="utf-8"), files)
    except Refusal as refusal:
        print(f"aur-render: refused: {refusal}", file=sys.stderr)
        return 1
    if shutil.which("makepkg") is None:
        print("aur-render: makepkg is not available; .SRCINFO cannot be generated", file=sys.stderr)
        return 1
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "PKGBUILD").write_text(rendered, encoding="utf-8")
    srcinfo = subprocess.run(
        ["makepkg", "--printsrcinfo"], cwd=output_dir, capture_output=True, text=True, check=False
    )
    if srcinfo.returncode != 0:
        print(f"aur-render: makepkg --printsrcinfo failed:\n{srcinfo.stderr}", file=sys.stderr)
        return 1
    (output_dir / ".SRCINFO").write_text(srcinfo.stdout, encoding="utf-8")
    print(f"aur-render: wrote {output_dir / 'PKGBUILD'} and {output_dir / '.SRCINFO'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
