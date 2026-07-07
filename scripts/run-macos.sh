#!/bin/sh
# scripts/run-macos.sh
#
# Removes the macOS quarantine attribute from the bekoedit binary so
# Gatekeeper allows it to run without a paid Apple Developer certificate.
#
# Usage (run once after downloading the release archive):
#   chmod +x scripts/run-macos.sh
#   ./scripts/run-macos.sh
#
# What this does:
#   xattr -cr ./bekoedit
#   Strips the com.apple.quarantine extended attribute that macOS adds to
#   files downloaded from the internet. This is equivalent to right-clicking
#   the app in Finder and choosing "Open", then confirming the dialog.
#
# This does NOT disable Gatekeeper system-wide — it only affects this binary.

set -e

BIN="${1:-./bekoedit}"

if [ ! -f "$BIN" ]; then
  echo "Error: binary not found at $BIN"
  echo "Usage: $0 [path-to-bekoedit]"
  exit 1
fi

echo "Removing quarantine attribute from $BIN ..."
xattr -cr "$BIN"
echo "Done. You can now launch bekoedit:"
echo "  $BIN"
