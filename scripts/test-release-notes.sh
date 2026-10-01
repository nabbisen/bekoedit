#!/usr/bin/env bash
# Self-test for scripts/release-notes.py: the CHANGELOG section is copied
# verbatim, and the Linux known-issue note appears exactly when, and only
# when, scripts/linux-portability-exemptions.tsv's Arch row is non-empty --
# after the section, before the closing "---"/"Full history:" footer.
# Run from the repo root: bash scripts/test-release-notes.sh
#
# Each case builds its own throwaway directory under a temp directory, with
# its own CHANGELOG.md and exemptions.tsv, so nothing in the real tree is
# touched and this never depends on CHANGELOG.md's actual current content.

set -u
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NOTES="$SCRIPT_DIR/release-notes.py"
FAILURES=0
CASES=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

CHANGELOG_BODY='### Added
- a thing'

NOTE_HEADING='### Known issue (Linux)'

# fixture <case-name> <arch-exempt: yes|no>: a throwaway copy of
# release-notes.py (so a mutation can be applied to it) with its own
# CHANGELOG.md and exemptions.tsv alongside it, at the relative path the
# script expects (its own directory).
fixture() {
  local dir="$WORK/$1"
  mkdir -p "$dir"
  cp "$NOTES" "$dir/release-notes.py"
  printf 'arch\t%s\nfedora\t\n' "$([ "$2" = yes ] && echo libxdo.so.3)" \
    >"$dir/linux-portability-exemptions.tsv"
  cat >"$dir/CHANGELOG.md" <<EOF
# Changelog

## [Unreleased]

## [1.2.3] - 2026-01-01

$CHANGELOG_BODY

[Unreleased]: https://example.invalid/compare/1.2.3...HEAD
[1.2.3]: https://example.invalid/releases/1.2.3
EOF
  echo "$dir"
}

run() { # <dir>
  (cd "$1" && python3 release-notes.py 1.2.3 CHANGELOG.md example/repo 1.2.3) 2>&1
}

expect_note_after_section_before_footer() { # <case-name> <dir>
  CASES=$((CASES + 1))
  local out note_at footer_at
  out=$(run "$2")
  note_at=$(printf '%s' "$out" | grep -n -F "$NOTE_HEADING" | head -1 | cut -d: -f1)
  footer_at=$(printf '%s' "$out" | grep -n -F -- '---' | head -1 | cut -d: -f1)
  if [ -n "$note_at" ] && [ -n "$footer_at" ] && [ "$note_at" -lt "$footer_at" ] \
    && printf '%s' "$out" | grep -qF "$CHANGELOG_BODY"; then
    echo "ok:   $1"
  else
    echo "FAIL: $1 -- expected the note after the section and before the footer:"
    printf '%s' "$out" | sed 's/^/        /'
    FAILURES=$((FAILURES + 1))
  fi
}

expect_no_note() { # <case-name> <dir>
  CASES=$((CASES + 1))
  local out
  out=$(run "$2")
  if ! printf '%s' "$out" | grep -qF "$NOTE_HEADING" \
    && printf '%s' "$out" | grep -qF "$CHANGELOG_BODY"; then
    echo "ok:   $1"
  else
    echo "FAIL: $1 -- expected no known-issue note:"
    printf '%s' "$out" | sed 's/^/        /'
    FAILURES=$((FAILURES + 1))
  fi
}

d=$(fixture exempt yes)
expect_note_after_section_before_footer "Arch exemption active: the note is printed, after the section, before the footer" "$d"

d=$(fixture not-exempt no)
expect_no_note "Arch exemption emptied: no note, and nothing else changes" "$d"

echo ""
echo "test-release-notes: $CASES case(s), $FAILURES failure(s)"
[ "$FAILURES" -eq 0 ]
