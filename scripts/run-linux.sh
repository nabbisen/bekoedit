#!/bin/sh
# scripts/run-linux.sh
#
# Checks that every shared library the bekoedit binary needs actually
# resolves on this system, and reports by name any that do not.
#
# Usage (run once after downloading the release archive, before launching):
#   chmod +x run-linux.sh
#   ./run-linux.sh
#
# Or against a specific path:
#   ./run-linux.sh /path/to/bekoedit
#
# What this does:
#   Runs `ldd` against the binary and reports any library it needs that
#   resolves to "not found". A missing library makes the binary fail to
#   launch — and launched from a desktop environment that failure is
#   SILENT: no window, no dialog, no log message, because the dynamic
#   loader gives up before any bekoedit code runs to report it. This
#   script surfaces the problem before you hit that silence.
#
# There are two Linux archives (RFC-045 slice 3a), and a missing libxdo is
# how a wrong one shows:
#   - libxdo.so.3 missing: this is the default Linux archive, on a system
#     that ships libxdo.so.4 (Arch and Arch-based distributions). Use the
#     "-arch" archive instead.
#   - libxdo.so.4 missing: this is the "-arch" archive, on a system that
#     ships libxdo.so.3 (Debian, Ubuntu, Fedora and most others). Use the
#     default Linux archive instead.
#
# This does NOT fix anything, and does not attempt to. In particular:
# a missing libxdo.so.3 is not always fixable by installing a package.
# The `xdotool` package provides libxdo.so.3 on distributions that ship
# that SONAME — but some distributions (Arch and Arch-family systems,
# confirmed) ship libxdo.so.4 only. A .so.3 requirement is a SONAME/ABI
# break there, not a missing package, and no symlink or workaround is
# advised or supported. On those systems use the "-arch" archive, or build
# from source:
#   cargo install bekoedit

set -e

BIN="${1:-./bekoedit}"

if [ ! -f "$BIN" ]; then
  echo "Error: binary not found at $BIN"
  echo "Usage: $0 [path-to-bekoedit]"
  exit 1
fi

if ! command -v ldd >/dev/null 2>&1; then
  echo "Error: ldd not found. Install your distribution's glibc/binutils"
  echo "package and retry."
  exit 1
fi

MISSING=$(ldd "$BIN" 2>&1 | grep 'not found' || true)

if [ -z "$MISSING" ]; then
  echo "All shared libraries required by $BIN resolve on this system."
  echo "You can launch it:"
  echo "  $BIN"
  exit 0
fi

echo "The following libraries required by $BIN are missing on this system:"
echo "$MISSING" | sed 's/^\s*/  /'
echo

if echo "$MISSING" | grep -q 'libxdo\.so\.3'; then
  echo "libxdo.so.3 comes from the 'xdotool' package on distributions that"
  echo "ship that SONAME. If your distribution ships libxdo.so.4 instead"
  echo "(Arch and Arch-family systems, confirmed), installing xdotool will"
  echo "NOT help: a .so.3 requirement is a SONAME/ABI break there, not a"
  echo "missing package. Do not symlink libxdo.so.4 to libxdo.so.3 — that"
  echo "defeats a versioning guarantee on a guess. Use this release's Arch"
  echo "archive instead (bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz),"
  echo "or build from source, which links against whatever libxdo your"
  echo "system has:"
  echo "  cargo install bekoedit"
  echo
fi

if echo "$MISSING" | grep -q 'libxdo\.so\.4'; then
  echo "libxdo.so.4 is what the Arch archive needs. This system does not"
  echo "ship it, so this is the Arch archive on a system that ships"
  echo "libxdo.so.3 (Debian, Ubuntu, Fedora and most others): use the"
  echo "default Linux archive instead (bekoedit-<version>-x86_64-unknown-linux-gnu.tar.gz)."
  echo "Do not symlink libxdo.so.3 to libxdo.so.4."
  echo
fi

echo "This binary will fail to launch. From a desktop environment that"
echo "failure is SILENT — no window, dialog, or log message — because the"
echo "dynamic loader gives up before bekoedit's own code runs."
exit 1
