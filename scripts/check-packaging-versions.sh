#!/usr/bin/env bash
# Task 046 §2.5: the packaging files under packaging/ declare their version
# by hand (no release-time automation builds or publishes either package),
# so nothing else keeps them in step with the workspace version except this
# check. Unlike some other projects, this one has no post-release version
# bump: the workspace version at any commit on `main` is already the right
# packaging version, so a plain equality check is correct at every commit,
# not only at a tagged release.
#
# Fails, naming the file and both values, unless:
#   - packaging/windows/AppxManifest.xml's Identity Version is "<W>.0";
#   - packaging/linux/PKGBUILD has pkgver=<W> and pkgrel=1.
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

fail() {
  echo "FAIL [$1]: $2"
  ERRORS=$((ERRORS + 1))
}

if [ ! -f "$CARGO_TOML" ] || [ ! -f "$APPX" ] || [ ! -f "$PKGBUILD" ]; then
  echo "check-packaging-versions: missing one of $CARGO_TOML, $APPX, $PKGBUILD (run from the repo root)"
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
read_pkgbuild_var() {
  (
    set +u
    # shellcheck disable=SC1090
    source "$PKGBUILD" >/dev/null 2>&1
    printf '%s' "${!1-}"
  )
}
PKG_VER="$(read_pkgbuild_var pkgver)"
PKG_REL="$(read_pkgbuild_var pkgrel)"

if [ -z "$PKG_VER" ] && [ -z "$PKG_REL" ]; then
  fail pkgbuild-unparseable "$PKGBUILD: could not read pkgver or pkgrel (sourcing it produced neither)"
else
  if [ "$PKG_VER" = "$WORKSPACE_VERSION" ]; then
    echo "ok: $PKGBUILD pkgver is $PKG_VER"
  else
    fail pkgbuild-version-mismatch "$PKGBUILD has pkgver=$PKG_VER, expected pkgver=$WORKSPACE_VERSION (the workspace version)"
  fi
  if [ "$PKG_REL" = "1" ]; then
    echo "ok: $PKGBUILD pkgrel is 1"
  else
    fail pkgbuild-pkgrel-not-one "$PKGBUILD has pkgrel=$PKG_REL, expected pkgrel=1"
  fi
fi

echo ""
echo "check-packaging-versions result: $ERRORS error(s)"
[ "$ERRORS" -eq 0 ]
