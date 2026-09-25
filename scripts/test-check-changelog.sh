#!/usr/bin/env bash
# Self-test for scripts/check-changelog.sh: a small valid tree passes, and each
# rule fails, by name, on a tree broken in exactly that way.
# Run from the repo root: bash scripts/test-check-changelog.sh
#
# Each case builds its own throwaway git repository under a temp directory, so
# nothing in the real tree is touched.

set -u
CHECK="$(cd "$(dirname "$0")" && pwd)/check-changelog.sh"
FAILURES=0
CASES=0
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# A valid tree: the current series (0.10-0.19) in CHANGELOG.md, an archive for
# 0.1-0.9, the archive linked, and links that resolve.
fixture() {
  local dir="$WORK/$1"
  mkdir -p "$dir/changelog" "$dir/docs"
  cat >"$dir/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

## [0.11.0] - 2026-06-07

### Added
- a

### Added
- b

[Unreleased]: https://example.invalid/compare/0.11.0...HEAD
[0.11.0]: https://example.invalid/releases/0.11.0

Older releases:

- [0.1 to 0.9](changelog/0.1-0.9.md)
EOF
  cat >"$dir/changelog/0.1-0.9.md" <<'EOF'
# Changelog: 0.1 to 0.9

## [0.9.0] - 2026-06-07

### Added
- c

[0.9.0]: https://example.invalid/releases/0.9.0
EOF
  cat >"$dir/docs/a.md" <<'EOF'
See [the latest](../CHANGELOG.md#0110---2026-06-07), [the second Added](../CHANGELOG.md#added-1)
and [an old one](../changelog/0.1-0.9.md#090---2026-06-07).
EOF
  echo "$dir"
}

# run <dir>: the check's output and exit code, in a git repository.
run() {
  (cd "$1" && git init -q . 2>/dev/null && git add -A 2>/dev/null && bash "$CHECK") 2>&1
}

expect_pass() { # <case name> <dir>
  CASES=$((CASES + 1))
  local out
  out=$(run "$2")
  if [ $? -eq 0 ] && echo "$out" | grep -q '0 error(s)'; then
    echo "ok:   $1"
  else
    echo "FAIL: $1 -- expected a pass:"
    echo "$out" | sed 's/^/        /'
    FAILURES=$((FAILURES + 1))
  fi
}

expect_fail() { # <case name> <rule> <dir>
  CASES=$((CASES + 1))
  local out
  out=$(run "$3")
  if [ $? -ne 0 ] && echo "$out" | grep -q "FAIL \[$2\]"; then
    echo "ok:   $1 fails as [$2]"
  else
    echo "FAIL: $1 -- expected FAIL [$2]:"
    echo "$out" | sed 's/^/        /'
    FAILURES=$((FAILURES + 1))
  fi
}

d=$(fixture baseline)
expect_pass "a valid tree" "$d"

d=$(fixture fenced)
printf '\n```\n[x](../CHANGELOG.md#nope) and [y](../changelog/gone.md)\n```\n' >>"$d/docs/a.md"
expect_pass "links inside a fenced code block are ignored" "$d"

d=$(fixture no-archive)
rm -rf "$d/changelog"
sed -i '/^Older releases:/,$d; /^\[0.1 to 0.9\]/d' "$d/CHANGELOG.md"
sed -i 's|../changelog/0.1-0.9.md#090---2026-06-07|../CHANGELOG.md#added|' "$d/docs/a.md"
printf '## [0.9.0] - 2026-06-07\n\n### Added\n- c\n\n[0.9.0]: https://example.invalid/0.9.0\n' >>"$d/CHANGELOG.md"
expect_pass "before the first archive, an old series in CHANGELOG.md is allowed to run" "$d"

d=$(fixture dup)
printf '\n## [0.9.0] - 2026-06-07\n\n[0.9.0]: https://example.invalid/x\n' >>"$d/CHANGELOG.md"
expect_fail "a version in both files" duplicate-version "$d"

d=$(fixture unlinked)
sed -i '/^Older releases:/,$d' "$d/CHANGELOG.md"
expect_fail "an archive file not linked" archive-unlinked "$d"

d=$(fixture broken)
echo 'and [gone](../changelog/0.20-0.29.md)' >>"$d/docs/a.md"
expect_fail "a link to a file that does not exist" link-broken "$d"

d=$(fixture broken-from-changelog)
echo '- [gone](changelog/0.20-0.29.md)' >>"$d/CHANGELOG.md"
expect_fail "CHANGELOG.md links to an archive that does not exist" link-broken "$d"

d=$(fixture moved)
echo '[moved](../CHANGELOG.md#090---2026-06-07)' >>"$d/docs/a.md"
expect_fail "a repository link to a section that moved" anchor-missing "$d"

d=$(fixture repeat-heading)
echo '[third](../CHANGELOG.md#added-2)' >>"$d/docs/a.md"
expect_fail "a link to a repeated heading's number that does not exist" anchor-missing "$d"

d=$(fixture url)
echo '[url](https://github.com/nabbisen/bekoedit/blob/main/CHANGELOG.md#nope)' >>"$d/docs/a.md"
expect_fail "a github.com/.../blob/main URL to a missing section" anchor-missing "$d"

d=$(fixture url-tag)
echo '[pinned](https://github.com/nabbisen/bekoedit/blob/0.9.0/CHANGELOG.md#nope)' >>"$d/docs/a.md"
expect_pass "a link pinned to a tag is not this check's business" "$d"

d=$(fixture old)
printf '\n## [0.9.1] - 2026-06-07\n\n[0.9.1]: https://example.invalid/x\n' >>"$d/CHANGELOG.md"
expect_fail "an old-series version left in CHANGELOG.md" old-series "$d"

d=$(fixture misnamed)
mv "$d/changelog/0.1-0.9.md" "$d/changelog/0.1-0.8.md"
sed -i 's|changelog/0.1-0.9.md|changelog/0.1-0.8.md|' "$d/CHANGELOG.md" "$d/docs/a.md"
expect_fail "a misnamed archive file" archive-name "$d"

d=$(fixture wrong-content)
printf '\n## [0.10.0] - 2026-06-07\n\n[0.10.0]: https://example.invalid/x\n' >>"$d/changelog/0.1-0.9.md"
expect_fail "an archive holding another series' version" archive-content "$d"

d=$(fixture current-archived)
mv "$d/changelog/0.1-0.9.md" "$d/changelog/0.10-0.19.md"
sed -i 's|changelog/0.1-0.9.md|changelog/0.10-0.19.md|' "$d/CHANGELOG.md" "$d/docs/a.md"
expect_fail "the current series filed as an archive" archive-content "$d"

d=$(fixture no-unreleased)
sed -i '/^## \[Unreleased\]/d' "$d/CHANGELOG.md"
expect_fail "no [Unreleased] heading" unreleased "$d"

d=$(fixture unreleased-in-archive)
printf '\n## [Unreleased]\n' >>"$d/changelog/0.1-0.9.md"
expect_fail "[Unreleased] in an archive" unreleased "$d"

d=$(fixture malformed)
printf '\n## [0.11.1] 2026-06-07\n' >>"$d/CHANGELOG.md"
expect_fail "a version heading with no dash" malformed-heading "$d"

d=$(fixture misplaced-def)
sed -i '/^\[0.9.0\]:/d' "$d/changelog/0.1-0.9.md"
sed -i 's|^\[0.11.0\]:.*|&\n[0.9.0]: https://example.invalid/releases/0.9.0|' "$d/CHANGELOG.md"
expect_fail "a link definition left behind in the other file" definition-placement "$d"

d=$(fixture slug)
printf '\n## Keeping it *short* (and `sweet`)!\n' >>"$d/CHANGELOG.md"
echo '[slug](../CHANGELOG.md#keeping-it-short-and-sweet)' >>"$d/docs/a.md"
expect_pass "GitHub's slug rule: punctuation removed, spaces become hyphens" "$d"

echo ""
echo "test-check-changelog: $CASES case(s), $FAILURES failure(s)"
[ "$FAILURES" -eq 0 ]
