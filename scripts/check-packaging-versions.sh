#!/usr/bin/env bash
# Task 046 §2.5: the packaging files under packaging/ declare their version
# by hand, so nothing else keeps them in step with the workspace version except
# this check. Unlike some other projects, this one has no post-release version
# bump: the workspace version at any commit on `main` is already the right
# packaging version, so a plain equality check is correct at every commit,
# not only at a tagged release.
#
# No release-time automation builds or publishes the Microsoft Store package;
# the owner does, by hand. The two AUR packages are different (RFC-045 slice 3b):
# CI renders them from the two PKGBUILD templates below at each tag, with
# scripts/aur-render.py, which itself refuses a template whose pkgver is not
# the tag. This check is what keeps the templates equal to the workspace
# version before the tag is made.
#
# Fails, naming the file and both values, unless:
#   - packaging/windows/AppxManifest.xml's Identity Version is "<W>.0";
#   - packaging/linux/PKGBUILD has pkgver=<W> and pkgrel=1;
#   - packaging/linux/bin/PKGBUILD (bekoedit-bin) has pkgver=<W> and pkgrel=1.
#
# Neither file is read with a grep a reformat could silently defeat: the
# manifest is parsed as XML (python3's stdlib ElementTree), and the
# PKGBUILD is parsed by sourcing it in a subshell and reading its own
# variables -- the one reader that can never disagree with what `makepkg`
# itself would see. An unparseable file reads as an empty value, which
# fails the equality check below by name; it is never read as a pass.
#
# Run from the repo root: bash scripts/check-packaging-versions.sh

set -u
export LC_ALL=C
ERRORS=0

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_TOML="Cargo.toml"
APPX="packaging/windows/AppxManifest.xml"
PKGBUILD="packaging/linux/PKGBUILD"
PKGBUILD_BIN="packaging/linux/bin/PKGBUILD"

fail() {
  echo "FAIL [$1]: $2"
  ERRORS=$((ERRORS + 1))
}

if [ ! -f "$CARGO_TOML" ] || [ ! -f "$APPX" ] || [ ! -f "$PKGBUILD" ] || [ ! -f "$PKGBUILD_BIN" ]; then
  echo "check-packaging-versions: missing one of $CARGO_TOML, $APPX, $PKGBUILD, $PKGBUILD_BIN (run from the repo root)"
  exit 1
fi

# Same anchored read scripts/publish-crates.sh already uses for the
# workspace version: anchored at the start of the line, so it can only
# match [workspace.package]'s own `version = "..."` line, never a member
# crate's `version.workspace = true`.
WORKSPACE_VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)"[[:space:]]*$/\1/p' "$CARGO_TOML" | head -n1)"
if [ -z "$WORKSPACE_VERSION" ]; then
  fail cargo-toml-unparseable "could not read [workspace.package] version from $CARGO_TOML"
  echo ""
  echo "check-packaging-versions result: $ERRORS error(s)"
  exit 1
fi
echo "ok: workspace version is $WORKSPACE_VERSION"

# ------------------------------------------------------------- AppxManifest
APPX_VERSION="$(python3 - "$APPX" <<'PY'
import sys
import xml.etree.ElementTree as ET

path = sys.argv[1]
try:
    root = ET.parse(path).getroot()
except ET.ParseError as error:
    print(f"UNPARSEABLE: {error}")
    raise SystemExit(0)

identity = root.find("{*}Identity")
if identity is None or "Version" not in identity.attrib:
    print("UNPARSEABLE: no Identity element with a Version attribute")
else:
    print(identity.attrib["Version"])
PY
)"

EXPECTED_APPX_VERSION="$WORKSPACE_VERSION.0"
case "$APPX_VERSION" in
  UNPARSEABLE*)
    fail appx-unparseable "$APPX: ${APPX_VERSION#UNPARSEABLE: }"
    ;;
  "$EXPECTED_APPX_VERSION")
    echo "ok: $APPX Identity Version is $APPX_VERSION"
    ;;
  *)
    fail appx-version-mismatch "$APPX has Identity Version=\"$APPX_VERSION\", expected \"$EXPECTED_APPX_VERSION\" (workspace version $WORKSPACE_VERSION plus .0)"
    ;;
esac

# ------------------------------------------------------------------ PKGBUILD
# Sourced in a subshell, not grepped: the one reader that can never
# disagree with what `makepkg` itself would see for these two variables,
# whatever the file's own formatting or comments look like.
read_pkgbuild_var() { # <file> <variable>
  (
    set +u
    # shellcheck disable=SC1090
    source "$1" >/dev/null 2>&1
    printf '%s' "${!2-}"
  )
}

check_pkgbuild() { # <file>
  local file="$1" pkg_ver pkg_rel
  pkg_ver="$(read_pkgbuild_var "$file" pkgver)"
  pkg_rel="$(read_pkgbuild_var "$file" pkgrel)"
  if [ -z "$pkg_ver" ] && [ -z "$pkg_rel" ]; then
    fail pkgbuild-unparseable "$file: could not read pkgver or pkgrel (sourcing it produced neither)"
    return
  fi
  if [ "$pkg_ver" = "$WORKSPACE_VERSION" ]; then
    echo "ok: $file pkgver is $pkg_ver"
  else
    fail pkgbuild-version-mismatch "$file has pkgver=$pkg_ver, expected pkgver=$WORKSPACE_VERSION (the workspace version)"
  fi
  if [ "$pkg_rel" = "1" ]; then
    echo "ok: $file pkgrel is 1"
  else
    fail pkgbuild-pkgrel-not-one "$file has pkgrel=$pkg_rel, expected pkgrel=1"
  fi
}

check_pkgbuild "$PKGBUILD"
check_pkgbuild "$PKGBUILD_BIN"

echo ""
echo "check-packaging-versions result: $ERRORS error(s)"
[ "$ERRORS" -eq 0 ]
