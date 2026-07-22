#!/usr/bin/env bash
set -euo pipefail

exec python3 - "$@" <<'PY'
from __future__ import annotations

import hashlib
import pathlib
import re
import stat
import sys
import tarfile
import zipfile


TARGETS = (
    ("x86_64-unknown-linux-gnu", ".tar.gz", "bekoedit"),
    ("aarch64-apple-darwin", ".tar.gz", "bekoedit"),
    ("x86_64-pc-windows-msvc", ".zip", "bekoedit.exe"),
)
DOCUMENTS = ("README.md", "LICENSE", "NOTICE", "CHANGELOG.md")


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"release artifact verification failed: {message}")


def validate_member_name(archive: pathlib.Path, name: str) -> None:
    if not name:
        fail(f"{archive.name}: empty archive member name")
    if name.startswith("/"):
        fail(f"{archive.name}: absolute archive member: {name!r}")
    if "\\" in name:
        fail(f"{archive.name}: backslash in archive member: {name!r}")
    if any(ord(character) < 32 or ord(character) == 127 for character in name):
        fail(f"{archive.name}: control character in archive member: {name!r}")
    if ".." in pathlib.PurePosixPath(name).parts:
        fail(f"{archive.name}: traversal archive member: {name!r}")


def validate_names(
    archive: pathlib.Path, names: list[str], expected_members: set[str]
) -> None:
    for name in names:
        validate_member_name(archive, name)
    duplicates = sorted({name for name in names if names.count(name) > 1})
    if duplicates:
        fail(f"{archive.name}: duplicate archive members: {duplicates}")
    actual = set(names)
    if actual != expected_members:
        missing = sorted(expected_members - actual)
        unexpected = sorted(actual - expected_members)
        fail(
            f"{archive.name}: member mismatch; missing={missing}, "
            f"unexpected={unexpected}"
        )


def validate_tar(archive: pathlib.Path, expected_members: set[str]) -> None:
    try:
        with tarfile.open(archive, mode="r:gz") as opened:
            members = opened.getmembers()
    except (OSError, tarfile.TarError) as error:
        fail(f"{archive.name}: invalid gzip/TAR archive: {error}")
    for member in members:
        if member.issym() or member.islnk():
            fail(f"{archive.name}: link member is forbidden: {member.name!r}")
        if not member.isfile():
            fail(
                f"{archive.name}: non-regular member is forbidden: "
                f"{member.name!r} (type={member.type!r})"
            )
    validate_names(archive, [member.name for member in members], expected_members)


def validate_zip(archive: pathlib.Path, expected_members: set[str]) -> None:
    try:
        with zipfile.ZipFile(archive, mode="r") as opened:
            members = opened.infolist()
            bad_member = opened.testzip()
    except (OSError, zipfile.BadZipFile, RuntimeError) as error:
        fail(f"{archive.name}: invalid ZIP archive: {error}")
    if bad_member is not None:
        fail(f"{archive.name}: corrupt ZIP member: {bad_member!r}")
    for member in members:
        unix_mode = (member.external_attr >> 16) & 0xFFFF
        if member.create_system == 3 and not stat.S_ISREG(unix_mode):
            fail(
                f"{archive.name}: Unix-created member is not a regular file: "
                f"{member.filename!r} (mode={unix_mode:#06o})"
            )
        if member.is_dir():
            fail(f"{archive.name}: directory member is forbidden: {member.filename!r}")
        if member.flag_bits & 0x1:
            fail(f"{archive.name}: encrypted member is forbidden: {member.filename!r}")
    validate_names(
        archive, [member.filename for member in members], expected_members
    )


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_sidecar(archive: pathlib.Path, sidecar: pathlib.Path) -> None:
    try:
        record = sidecar.read_bytes().decode("ascii")
    except (OSError, UnicodeDecodeError) as error:
        fail(f"{sidecar.name}: unreadable ASCII checksum record: {error}")
    match = re.fullmatch(
        rf"([0-9a-f]{{64}})  {re.escape(archive.name)}\r?\n", record
    )
    if match is None:
        fail(
            f"{sidecar.name}: expected one lowercase SHA-256 record for "
            f"{archive.name}"
        )
    actual = sha256(archive)
    if match.group(1) != actual:
        fail(
            f"{sidecar.name}: checksum mismatch for {archive.name}; "
            f"expected {match.group(1)}, calculated {actual}"
        )


def main() -> None:
    if len(sys.argv) != 3:
        fail("usage: check-release-artifacts.sh EXPECTED_VERSION ARTIFACT_DIR")
    version = sys.argv[1]
    if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        fail(f"invalid bare SemVer version: {version!r}")

    artifact_dir = pathlib.Path(sys.argv[2])
    if artifact_dir.is_symlink() or not artifact_dir.is_dir():
        fail(f"artifact directory is missing, not a directory, or a symlink: {artifact_dir}")

    expected_names: set[str] = set()
    archives: list[tuple[pathlib.Path, pathlib.Path, str]] = []
    for target, suffix, executable in TARGETS:
        archive = artifact_dir / f"bekoedit-{version}-{target}{suffix}"
        sidecar = artifact_dir / f"{archive.name}.sha256"
        expected_names.update((archive.name, sidecar.name))
        archives.append((archive, sidecar, executable))

    entries = list(artifact_dir.iterdir())
    unsafe_entries = sorted(
        entry.name for entry in entries if entry.is_symlink() or not entry.is_file()
    )
    if unsafe_entries:
        fail(f"artifact directory contains non-regular entries: {unsafe_entries}")
    actual_names = {entry.name for entry in entries}
    if len(entries) != 6 or actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        unexpected = sorted(actual_names - expected_names)
        fail(
            f"artifact directory must contain exactly six expected files; "
            f"missing={missing}, unexpected={unexpected}, count={len(entries)}"
        )

    for archive, sidecar, executable in archives:
        validate_sidecar(archive, sidecar)
        expected_members = {executable, *DOCUMENTS}
        if archive.name.endswith(".tar.gz"):
            validate_tar(archive, expected_members)
        else:
            validate_zip(archive, expected_members)

    print(f"verified six release files for bekoedit {version} in {artifact_dir}")


if __name__ == "__main__":
    main()
PY
