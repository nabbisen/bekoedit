#!/usr/bin/env bash
# RFC-000 §13 optional CI invariant checker.
# Run from the repo root: bash scripts/check-rfcs.sh
ERRORS=0

# 1. Every RFC file in proposed/done/archive has a Status field.
while IFS= read -r -d '' f; do
  if ! grep -qE '^\*\*Status[.:]' "$f"; then
    echo "FAIL: missing Status in $f"
    ERRORS=$((ERRORS+1))
  fi
done < <(find rfcs/proposed rfcs/done rfcs/archive -name "*.md" -print0 2>/dev/null)
echo "ok: Status fields present"

# 2. Files in done/ carry "Implemented" in their Status line.
WARN=0
while IFS= read -r -d '' f; do
  if ! grep -q "Implemented" "$f"; then
    echo "WARN: done/ file lacks Implemented: $f"
    WARN=$((WARN+1))
  fi
done < <(find rfcs/done -name "RFC-*.md" -print0 2>/dev/null)
[ "$WARN" -eq 0 ] && echo "ok: done/ RFCs all marked Implemented"

# 3. No RFC number in more than one folder.
NUMS=$(find rfcs/proposed rfcs/done rfcs/archive -name "RFC-*.md" 2>/dev/null \
  | sed 's|.*/RFC-\([0-9]*\)-.*|\1|' | sort | uniq -d)
if [ -z "$NUMS" ]; then
  echo "ok: no duplicate RFC numbers"
else
  echo "FAIL: duplicate RFC numbers: $NUMS"
  ERRORS=$((ERRORS+1))
fi

# 4. Every RFC-NNN referenced in README.md exists on disk.
while read -r num; do
  count=$(find rfcs -name "RFC-${num}-*.md" 2>/dev/null | wc -l)
  if [ "$count" -eq 0 ]; then
    echo "FAIL: RFC-${num} in README.md but not on disk"
    ERRORS=$((ERRORS+1))
  fi
done < <(grep -oE 'RFC-[0-9]+' rfcs/README.md | grep -oE '[0-9]+' | sort -un)
echo "ok: README.md RFC references resolve"

echo ""
echo "check-rfcs result: $ERRORS error(s)"
[ "$ERRORS" -eq 0 ]
