#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: test-release-artifacts.sh TEMPORARY_PARENT" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 - "$1" "$SCRIPT_DIR/check-release-artifacts.sh" <<'PY'
from __future__ import annotations

import hashlib
import io
import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile


VERSION = "0.13.1"
TARGETS = (
    ("x86_64-unknown-linux-gnu", ".tar.gz", "bekoedit"),
    ("aarch64-apple-darwin", ".tar.gz", "bekoedit"),
    ("x86_64-pc-windows-msvc", ".zip", "bekoedit.exe"),
)
DOCUMENTS = ("README.md", "LICENSE", "NOTICE", "CHANGELOG.md")
PAYLOADS = {
    "bekoedit": b"linux-or-macos-fixture\n",
    "bekoedit.exe": b"windows-fixture\n",
    "README.md": b"readme\n",
    "LICENSE": b"license\n",
    "NOTICE": b"notice\n",
    "CHANGELOG.md": b"changelog\n",
}


def archive_path(directory: pathlib.Path, target: str, suffix: str) -> pathlib.Path:
    return directory / f"bekoedit-{VERSION}-{target}{suffix}"


def write_sidecar(archive: pathlib.Path, digest: str | None = None) -> None:
    value = digest or hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(f"{archive.name}.sha256").write_text(
        f"{value}  {archive.name}\n", encoding="ascii"
    )


def write_tar(archive: pathlib.Path, members: list[tuple[str, str, bytes]]) -> None:
    with tarfile.open(archive, mode="w:gz") as opened:
        for name, kind, content in members:
            info = tarfile.TarInfo(name)
            info.mtime = 0
            if kind == "file":
                info.mode = 0o755 if name.endswith("bekoedit") else 0o644
                info.size = len(content)
                opened.addfile(info, io.BytesIO(content))
            elif kind == "symlink":
                info.type = tarfile.SYMTYPE
                info.linkname = "README.md"
                opened.addfile(info)
            elif kind == "hardlink":
                info.type = tarfile.LNKTYPE
                info.linkname = "README.md"
                opened.addfile(info)
            else:
                raise AssertionError(kind)


def write_zip(
    archive: pathlib.Path,
    members: list[tuple[str, str, bytes]],
    *,
    create_system: int = 3,
) -> None:
    with zipfile.ZipFile(archive, mode="w", compression=zipfile.ZIP_DEFLATED) as opened:
        for name, kind, content in members:
            info = zipfile.ZipInfo(name)
            info.create_system = create_system
            if kind == "file":
                if create_system == 3:
                    info.external_attr = (stat.S_IFREG | 0o644) << 16
                opened.writestr(info, content)
            elif kind == "symlink":
                info.external_attr = (stat.S_IFLNK | 0o777) << 16
                opened.writestr(info, "README.md")
            elif kind == "fifo":
                info.external_attr = (stat.S_IFIFO | 0o644) << 16
                opened.writestr(info, content)
            else:
                raise AssertionError(kind)


def normal_members(executable: str) -> list[tuple[str, str, bytes]]:
    return [
        *[(executable, "file", PAYLOADS[executable])],
        *[(name, "file", PAYLOADS[name]) for name in DOCUMENTS],
    ]


def create_valid(directory: pathlib.Path) -> None:
    directory.mkdir()
    for target, suffix, executable in TARGETS:
        archive = archive_path(directory, target, suffix)
        members = normal_members(executable)
        if suffix == ".tar.gz":
            write_tar(archive, members)
        else:
            # Match ordinary Windows/DOS-created ZIP metadata: no Unix type bits.
            write_zip(archive, members, create_system=0)
        write_sidecar(archive)


def rewrite_linux(directory: pathlib.Path, members: list[tuple[str, str, bytes]]) -> None:
    archive = archive_path(directory, TARGETS[0][0], TARGETS[0][1])
    write_tar(archive, members)
    write_sidecar(archive)


def rewrite_windows(directory: pathlib.Path, members: list[tuple[str, str, bytes]]) -> None:
    archive = archive_path(directory, TARGETS[2][0], TARGETS[2][1])
    write_zip(archive, members)
    write_sidecar(archive)


def invoke(verifier: pathlib.Path, directory: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(verifier), VERSION, str(directory)],
        check=False,
        capture_output=True,
        text=True,
    )


def expect_rejected(
    verifier: pathlib.Path,
    valid: pathlib.Path,
    run_root: pathlib.Path,
    name: str,
    mutation,
) -> None:
    case = run_root / name
    shutil.copytree(valid, case)
    mutation(case)
    result = invoke(verifier, case)
    if result.returncode == 0:
        raise SystemExit(f"negative fixture unexpectedly accepted: {name}")
    diagnostic = (result.stderr or result.stdout).strip().splitlines()
    detail = diagnostic[-1] if diagnostic else "no diagnostic"
    print(f"PASS reject {name}: {detail}")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("internal argument error")
    parent = pathlib.Path(sys.argv[1])
    verifier = pathlib.Path(sys.argv[2])
    parent.mkdir(parents=True, exist_ok=True)
    if parent.is_symlink() or not parent.is_dir():
        raise SystemExit(f"temporary parent is not a safe directory: {parent}")
    run_root = pathlib.Path(tempfile.mkdtemp(prefix="release-artifacts-", dir=parent))
    valid = run_root / "valid"
    create_valid(valid)

    accepted = invoke(verifier, valid)
    if accepted.returncode != 0:
        raise SystemExit(
            "valid fixture rejected:\n" + (accepted.stderr or accepted.stdout)
        )
    print(f"PASS accept valid: {accepted.stdout.strip()}")

    linux_members = normal_members("bekoedit")
    expect_rejected(
        verifier,
        valid,
        run_root,
        "missing-sidecar",
        lambda case: archive_path(case, TARGETS[0][0], TARGETS[0][1])
        .with_name(
            f"{archive_path(case, TARGETS[0][0], TARGETS[0][1]).name}.sha256"
        )
        .unlink(),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "extra-file",
        lambda case: (case / "unexpected.txt").write_text("extra"),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "wrong-checksum",
        lambda case: write_sidecar(
            archive_path(case, TARGETS[0][0], TARGETS[0][1]), "0" * 64
        ),
    )

    def mismatched_sidecar(case: pathlib.Path) -> None:
        linux = archive_path(case, TARGETS[0][0], TARGETS[0][1])
        macos = archive_path(case, TARGETS[1][0], TARGETS[1][1])
        write_sidecar(linux, hashlib.sha256(macos.read_bytes()).hexdigest())

    expect_rejected(verifier, valid, run_root, "mismatched-checksum", mismatched_sidecar)
    expect_rejected(
        verifier,
        valid,
        run_root,
        "enclosing-directory",
        lambda case: rewrite_linux(
            case, [(f"bekoedit-{VERSION}/{name}", kind, data) for name, kind, data in linux_members]
        ),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "absolute-member",
        lambda case: rewrite_linux(case, [("/bekoedit", "file", b"bad"), *linux_members[1:]]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "traversal-member",
        lambda case: rewrite_linux(case, [("../bekoedit", "file", b"bad"), *linux_members[1:]]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "symlink-member",
        lambda case: rewrite_linux(case, [("bekoedit", "symlink", b""), *linux_members[1:]]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "hardlink-member",
        lambda case: rewrite_linux(case, [("bekoedit", "hardlink", b""), *linux_members[1:]]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "zip-symlink-member",
        lambda case: rewrite_windows(
            case,
            [("bekoedit.exe", "symlink", b""), *normal_members("bekoedit.exe")[1:]],
        ),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "zip-fifo-member",
        lambda case: rewrite_windows(
            case,
            [("bekoedit.exe", "fifo", b"bad"), *normal_members("bekoedit.exe")[1:]],
        ),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "duplicate-member",
        lambda case: rewrite_linux(case, [*linux_members, linux_members[0]]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "unexpected-member",
        lambda case: rewrite_linux(case, [*linux_members, ("EXTRA", "file", b"extra")]),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "wrong-executable",
        lambda case: rewrite_linux(
            case, [("bekoedit.exe", "file", b"wrong"), *linux_members[1:]]
        ),
    )
    expect_rejected(
        verifier,
        valid,
        run_root,
        "control-character-member",
        lambda case: rewrite_linux(
            case, [("bekoedit\n", "file", b"bad"), *linux_members[1:]]
        ),
    )
    print(f"all release artifact verifier fixtures passed in {run_root}")


if __name__ == "__main__":
    main()
PY
