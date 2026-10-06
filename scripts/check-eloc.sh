#!/usr/bin/env bash
# ELOC compliance: no Rust file under crates/ may exceed ELOC_LIMIT effective
# lines (blank lines and `//` comment lines do not count). CI's "ELOC compliance
# check" step calls this script, so a local run is exactly the CI gate.
#
# Usage: scripts/check-eloc.sh          # ELOC_LIMIT defaults to 500
set -euo pipefail
ELOC_LIMIT="${ELOC_LIMIT:-500}"
cd "$(dirname "$0")/.."
OVER=""
while IFS= read -r f; do
  n=$(grep -vc "^[[:space:]]*$\|^[[:space:]]*//" "$f" 2>/dev/null || echo 0)
  if [ "$n" -gt "$ELOC_LIMIT" ]; then
    OVER="${OVER}${n} ${f}"$'\n'
  fi
done < <(find crates -name "*.rs" -type f)
if [ -n "$OVER" ]; then
  echo "Files over ${ELOC_LIMIT} ELOC:"
  printf "%s" "$OVER"
  exit 1
fi
echo "check-eloc: every file is within ${ELOC_LIMIT} ELOC"
