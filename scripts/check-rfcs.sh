#!/usr/bin/env bash
# RFC lifecycle invariant checker (see rfcs/done/000-rfc-lifecycle-policy.md,
# "Optional CI invariants"). Run from the repo root: bash scripts/check-rfcs.sh
#
# This project uses the policy's 5-folder variant: proposed/ accepted/ done/
# archive/ (no draft/). The folder is the source of truth for an RFC's state.
ERRORS=0
STATE_DIRS="rfcs/proposed rfcs/accepted rfcs/done rfcs/archive"

# 1. Every RFC file in a state folder has a Status field.
while IFS= read -r -d '' f; do
  if ! grep -qE '^\*\*Status[.:]' "$f"; then
    echo "FAIL: missing Status in $f"
    ERRORS=$((ERRORS+1))
  fi
done < <(find $STATE_DIRS -name "*.md" -print0 2>/dev/null)
echo "ok: Status fields present"

# 2. Status value matches the folder. The folder wins, so a disagreement is a
#    file that lies about itself.
check_state() {
  local dir="$1" want="$2" bad=0
  while IFS= read -r -d '' f; do
    if ! grep -qE "^\*\*Status[.:]\*\*[[:space:]]*$want" "$f"; then
      echo "FAIL: $dir file's Status is not '$want': $f"
      bad=$((bad+1))
      ERRORS=$((ERRORS+1))
    fi
  done < <(find "$dir" -name "RFC-*.md" -print0 2>/dev/null)
  [ "$bad" -eq 0 ] && echo "ok: $dir RFCs all marked $want"
}
check_state rfcs/proposed Proposed
check_state rfcs/accepted Accepted
check_state rfcs/done Implemented

# 3. No RFC number in more than one folder.
NUMS=$(find $STATE_DIRS -name "RFC-*.md" 2>/dev/null \
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

# 5. Every RFC on disk is linked from README.md at its current path.
#    Match the state-qualified path, not the RFC number: a number alone also
#    appears in other rows' prose, so grepping for it passes even when the RFC
#    has no row of its own -- and matching the path additionally catches an
#    index entry left pointing at the folder an RFC just moved out of.
MISSING_FROM_INDEX=0
while IFS= read -r -d '' f; do
  rel="${f#rfcs/}"
  if ! grep -qF "$rel" rfcs/README.md; then
    echo "FAIL: $f is not linked from rfcs/README.md at its current path"
    MISSING_FROM_INDEX=$((MISSING_FROM_INDEX+1))
    ERRORS=$((ERRORS+1))
  fi
done < <(find $STATE_DIRS -name "RFC-*.md" -print0 2>/dev/null)
[ "$MISSING_FROM_INDEX" -eq 0 ] && echo "ok: every RFC on disk is linked from the index"

# 6. Every relative Markdown link inside rfcs/ resolves. Moving an RFC between
#    folders is exactly what breaks these, so this is the check that earns its
#    keep on a lifecycle transition. Links inside fenced code blocks are
#    illustrations, not links -- the lifecycle policy is full of example paths
#    like ./done/010-revoke-tokens.md that are not meant to exist here.
BROKEN=0
while IFS= read -r -d '' f; do
  dir=$(dirname "$f")
  while read -r target; do
    [ -z "$target" ] && continue
    case "$target" in http*|\#*|mailto:*) continue ;; esac
    resolved="${target%%#*}"
    [ -z "$resolved" ] && continue
    if [ ! -e "$dir/$resolved" ]; then
      echo "FAIL: broken link in $f -> $target"
      BROKEN=$((BROKEN+1))
      ERRORS=$((ERRORS+1))
    fi
  done < <(awk '/^```/{fence=!fence; next} !fence' "$f" \
             | grep -oE '\]\([^)]+\)' | sed 's|^](||; s|)$||')
done < <(find rfcs -name "*.md" -print0 2>/dev/null)
[ "$BROKEN" -eq 0 ] && echo "ok: relative links inside rfcs/ resolve"

# 7. RFC 000 is a verbatim mirror of a policy shared across projects. Its
#    source lives outside the repository, so this can only run where that
#    source is present -- CI cannot see it. The skip is announced rather than
#    silent: a check that quietly stops running is worse than one that is
#    honestly unavailable.
POLICY_SRC=".git-exclude/rules/000-rfc-lifecycle-policy.md"
POLICY_MIRROR="rfcs/done/000-rfc-lifecycle-policy.md"
if [ ! -e "$POLICY_SRC" ]; then
  echo "skip: RFC 000 mirror check -- $POLICY_SRC not present (expected in CI)"
elif diff -q "$POLICY_SRC" "$POLICY_MIRROR" >/dev/null 2>&1; then
  echo "ok: RFC 000 mirrors its shared source verbatim"
else
  echo "FAIL: $POLICY_MIRROR has diverged from $POLICY_SRC"
  diff "$POLICY_SRC" "$POLICY_MIRROR" | head -20
  echo "      (RFC 000 is shared across projects -- sync by copying the source"
  echo "       over the mirror; project-specific notes belong in rfcs/README.md)"
  ERRORS=$((ERRORS+1))
fi

echo ""
echo "check-rfcs result: $ERRORS error(s)"
[ "$ERRORS" -eq 0 ]
