#!/usr/bin/env bash
# Changelog invariants (docs/src/changelog-policy.md, "What checks this").
# Run from the repo root: bash scripts/check-changelog.sh
#
# CHANGELOG.md holds the current series; finished series are moved, never
# copied, into one file each under changelog/. A move can fail in ways nobody
# notices, so this checks, by name:
#
#   duplicate-version    a version appears in more than one place
#   malformed-heading    a `## [` heading that is neither [Unreleased] nor a
#                        dated `## [x.y.z] - YYYY-MM-DD`
#   unreleased           [Unreleased] is missing, repeated, or in an archive
#   definition-placement a `[x.y.z]:` link definition is not in the file that
#                        holds that version's heading, or is defined twice
#   archive-name         a file in changelog/ is misnamed
#   archive-content      an archive holds a version outside its own series, or
#                        holds the current series
#   archive-unlinked     a file in changelog/ is not linked from CHANGELOG.md
#   link-broken          a link into CHANGELOG.md or changelog/ resolves to
#                        nothing
#   anchor-missing       a link to CHANGELOG.md#... or changelog/x.md#... names
#                        a section that is not there (from ANY tracked Markdown
#                        file, and including https://github.com/nabbisen/
#                        bekoedit/blob/main/... URLs)
#   old-series           CHANGELOG.md still holds a version of an older series
#
# Series: before 1.0.0, ten minor versions and their patches (0.1-0.9,
# 0.10-0.19, 0.20-0.29 ...); from 1.0.0, one major (1.x, 2.x ...). The current
# series is the one containing the newest dated version in CHANGELOG.md.
#
# Carve-out: `old-series` is enforced once a changelog/ directory exists. Before
# the first archive there is nothing for an old version to have moved to, and
# the check has to be able to run on that file.
#
# Anchors are computed the way GitHub does (see `anchors` below): lower-case,
# remove every ASCII character that is not a letter, digit, space, `_` or `-`,
# turn each space into `-`, and number repeats (`added`, `added-1`, `added-2`).
# Non-ASCII characters are kept. Examples:
#   "## [0.16.0] - 2026-09-25"  ->  #0160---2026-09-25
#   "### Added"                 ->  #added   (a second one: #added-1)
#   "## Keeping it *short*"     ->  #keeping-it-short
#
# Links inside fenced code blocks are illustrations, not links, and are skipped.

set -u
export LC_ALL=C
ERRORS=0
NOTES=0
CHANGELOG="CHANGELOG.md"
ARCHIVE_DIR="changelog"

fail() {
  echo "FAIL [$1]: $2"
  ERRORS=$((ERRORS + 1))
}
note() {
  echo "note: $1"
  NOTES=$((NOTES + 1))
}

if [ ! -f "$CHANGELOG" ]; then
  echo "FAIL [changelog-missing]: $CHANGELOG not found (run from the repo root)"
  exit 1
fi

ARCHIVES=()
if [ -d "$ARCHIVE_DIR" ]; then
  while IFS= read -r -d '' f; do ARCHIVES+=("$f"); done \
    < <(find "$ARCHIVE_DIR" -maxdepth 1 -type f -name '*.md' -print0 | sort -z)
fi
ALL=("$CHANGELOG" ${ARCHIVES[@]+"${ARCHIVES[@]}"})

# Everything below reads a file without its fenced code blocks.
unfenced() { awk '/^```/{fence=!fence; next} !fence' "$1"; }

# rows <file> <key>: the second column of every row whose first column is <key>.
rows() { awk -v key="$2" '$1 == key { print $2 }' "$1"; }

# ---------------------------------------------------------------- headings
# One line per dated version heading: "<version> <file>".
VERSIONS=$(mktemp)
ANCHORS=""
DEFS=""
trap 'rm -f "$VERSIONS" "$ANCHORS" "$DEFS" 2>/dev/null' EXIT
for f in "${ALL[@]}"; do
  unfenced "$f" | grep -E '^## \[[0-9]+\.[0-9]+\.[0-9]+\] - [0-9]{4}-[0-9]{2}-[0-9]{2}' \
    | sed -E 's/^## \[([0-9]+\.[0-9]+\.[0-9]+)\].*/\1/' | sed "s|\$| $f|" >>"$VERSIONS"
done

# 1. Each version appears exactly once across all the files.
DUPES=$(cut -d' ' -f1 "$VERSIONS" | sort | uniq -d)
if [ -z "$DUPES" ]; then
  echo "ok: each of the $(wc -l <"$VERSIONS" | tr -d ' ') versions appears exactly once"
else
  for v in $DUPES; do
    fail duplicate-version "$v is in: $(rows "$VERSIONS" "$v" | tr '\n' ' ')"
  done
fi

# 1b. A `## [` heading that is not [Unreleased] must be a dated version.
for f in "${ALL[@]}"; do
  while IFS= read -r line; do
    case "$line" in
      "## [Unreleased]") ;;
      *) echo "$line" | grep -qE '^## \[[0-9]+\.[0-9]+\.[0-9]+\] - [0-9]{4}-[0-9]{2}-[0-9]{2}' \
        || fail malformed-heading "$f: $line" ;;
    esac
  done < <(unfenced "$f" | grep -E '^## \[')
done

# 1c. [Unreleased]: once, in CHANGELOG.md, never in an archive.
UNRELEASED_IN_CHANGELOG=$(unfenced "$CHANGELOG" | grep -cx '## \[Unreleased\]')
[ "$UNRELEASED_IN_CHANGELOG" -eq 1 ] \
  || fail unreleased "$CHANGELOG has $UNRELEASED_IN_CHANGELOG '## [Unreleased]' headings, expected 1"
for f in ${ARCHIVES[@]+"${ARCHIVES[@]}"}; do
  if unfenced "$f" | grep -qx '## \[Unreleased\]'; then
    fail unreleased "$f holds '## [Unreleased]'; it belongs in $CHANGELOG"
  fi
done

# ------------------------------------------------- reference-link definitions
# One line per definition: "<version> <file>".
DEFS=$(mktemp)
for f in "${ALL[@]}"; do
  unfenced "$f" | grep -E '^\[[0-9]+\.[0-9]+\.[0-9]+\]:' \
    | sed -E 's/^\[([0-9]+\.[0-9]+\.[0-9]+)\]:.*/\1/' | sed "s|\$| $f|" >>"$DEFS"
done
BAD_DEF=0
while read -r v; do
  [ -z "$v" ] && continue
  where=$(rows "$VERSIONS" "$v")
  defs=$(rows "$DEFS" "$v")
  if [ -z "$defs" ]; then
    note "[$v] has no reference-link definition in any file, so its heading renders as literal brackets"
  elif [ "$(echo "$defs" | wc -l | tr -d ' ')" -gt 1 ]; then
    fail definition-placement "[$v] is defined more than once: $(echo "$defs" | tr '\n' ' ')"
    BAD_DEF=1
  elif [ "$defs" != "$where" ]; then
    fail definition-placement "[$v] is defined in $defs but its heading is in $where"
    BAD_DEF=1
  fi
done < <(cut -d' ' -f1 "$VERSIONS" | sort -u)
[ "$BAD_DEF" -eq 0 ] && echo "ok: every version's link definition is in the file that holds its heading"

# -------------------------------------------------------------------- series
# series_of <major> <minor>  ->  0.1-0.9 | 0.10-0.19 | ... | 1.x | 2.x
series_of() {
  local major=$1 minor=$2 lo
  if [ "$major" -ge 1 ]; then
    echo "${major}.x"
  elif [ "$minor" -lt 10 ]; then
    echo "0.1-0.9"
  else
    lo=$((minor / 10 * 10))
    echo "0.${lo}-0.$((lo + 9))"
  fi
}
series_of_version() { # <x.y.z>
  IFS=. read -r major minor _ <<<"$1"
  series_of "$major" "$minor"
}

CURRENT_SERIES=""
NEWEST=$(awk -v f="$CHANGELOG" '$2 == f { print $1 }' "$VERSIONS" | sort -V | tail -1)
[ -n "$NEWEST" ] && CURRENT_SERIES=$(series_of_version "$NEWEST")

# 2. Archive files: named by series, holding only that series, never the
#    current one.
for f in ${ARCHIVES[@]+"${ARCHIVES[@]}"}; do
  name=$(basename "$f" .md)
  want=""
  if [[ "$name" =~ ^0\.([0-9]+)-0\.([0-9]+)$ ]]; then
    lo=${BASH_REMATCH[1]}
    hi=${BASH_REMATCH[2]}
    if { [ "$lo" -eq 1 ] && [ "$hi" -eq 9 ]; } \
      || { [ "$lo" -ge 10 ] && [ $((lo % 10)) -eq 0 ] && [ "$hi" -eq $((lo + 9)) ]; }; then
      want="$name"
    fi
  elif [[ "$name" =~ ^[1-9][0-9]*\.x$ ]]; then
    want="$name"
  fi
  if [ -z "$want" ]; then
    fail archive-name "$f is not named for a series (0.1-0.9.md, 0.10-0.19.md, 0.20-0.29.md ..., 1.x.md, 2.x.md ...)"
    continue
  fi
  if [ "$want" = "$CURRENT_SERIES" ]; then
    fail archive-content "$f is the current series ($CURRENT_SERIES), which belongs in $CHANGELOG"
  fi
  while read -r v; do
    [ -z "$v" ] && continue
    got=$(series_of_version "$v")
    [ "$got" = "$want" ] \
      || fail archive-content "$f is the $want archive but holds $v, which is in series $got"
  done < <(awk -v f="$f" '$2 == f { print $1 }' "$VERSIONS")
done

# 3. Old-series versions must not stay in CHANGELOG.md (once an archive exists).
if [ -d "$ARCHIVE_DIR" ]; then
  OLD=0
  while read -r v; do
    [ -z "$v" ] && continue
    got=$(series_of_version "$v")
    if [ "$got" != "$CURRENT_SERIES" ]; then
      fail old-series "$v (series $got) is still in $CHANGELOG; the current series is $CURRENT_SERIES"
      OLD=$((OLD + 1))
    fi
  done < <(awk -v f="$CHANGELOG" '$2 == f { print $1 }' "$VERSIONS")
  [ "$OLD" -eq 0 ] && echo "ok: $CHANGELOG holds only the current series ($CURRENT_SERIES)"
else
  echo "skip: old-series -- no $ARCHIVE_DIR/ directory yet, so nothing an old version could have moved to"
fi

# ------------------------------------------------------------------ anchors
# One line per heading: "<file>\t<anchor>", GitHub-style, for CHANGELOG.md and
# every archive.
ANCHORS=$(mktemp)
anchors() { # <file>
  unfenced "$1" | awk -v file="$1" '
    /^#{1,6}[ \t]+/ {
      h = $0
      sub(/^#+[ \t]+/, "", h)
      sub(/[ \t]+#+[ \t]*$/, "", h)
      gsub(/\]\([^)]*\)/, "", h)          # [text](url) -> [text
      s = tolower(h)
      kept = ""
      for (i = 1; i <= length(s); i++) {
        c = substr(s, i, 1)
        if (c ~ /[a-z0-9 _-]/ || c >= "\200") kept = kept c
      }
      s = kept
      gsub(/ /, "-", s)
      n = seen[s]++
      print file "\t" (n == 0 ? s : s "-" n)
    }'
}
for f in "${ALL[@]}"; do anchors "$f" >>"$ANCHORS"; done

# normalize <path>: resolve `.` and `..` lexically.
normalize() {
  local IFS=/ part out=()
  for part in $1; do
    case "$part" in
      "" | .) ;;
      ..) [ "${#out[@]}" -gt 0 ] && unset 'out[${#out[@]}-1]' ;;
      *) out+=("$part") ;;
    esac
  done
  echo "${out[*]-}" | tr ' ' '/'
}

is_changelog_path() { [ "$1" = "$CHANGELOG" ] || [[ "$1" == $ARCHIVE_DIR/*.md ]]; }

# 4. Every link into CHANGELOG.md or changelog/ resolves, and every anchored one
#    names a section that is there. From any tracked Markdown file.
LINKS=0
TRACKED=()
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  while IFS= read -r -d '' f; do TRACKED+=("$f"); done < <(git ls-files -z '*.md')
else
  while IFS= read -r -d '' f; do TRACKED+=("${f#./}"); done \
    < <(find . -name '*.md' -not -path './.git/*' -print0)
fi
REPO_URL='https://github.com/nabbisen/bekoedit/blob/(main|HEAD)/'

# link_targets <file>: every link target in a file, outside fenced code blocks:
# inline `](target)` and reference definitions `[label]: target`.
link_targets() {
  unfenced "$1" | grep -oE '\]\([^)]+\)' | sed 's|^](||; s|)$||'
  unfenced "$1" | grep -E '^\[[^]]+\]:[ \t]+[^ \t]+' | sed -E 's/^\[[^]]+\]:[ \t]+([^ \t]+).*/\1/'
}

for f in "${TRACKED[@]}"; do
  dir=$(dirname "$f")
  while IFS= read -r target; do
    [ -z "$target" ] && continue
    target=${target%% *} # drop a "title"
    path=""
    frag=""
    case "$target" in
      \#*) path="$f"; frag="${target#\#}" ;;
      mailto:*) continue ;;
      http://* | https://*)
        if [[ "$target" =~ ^${REPO_URL}(.*)$ ]]; then
          rest=${BASH_REMATCH[2]}
          path=${rest%%#*}
          [[ "$rest" == *"#"* ]] && frag=${rest#*#}
        else
          continue
        fi
        ;;
      *)
        path=$(normalize "$dir/${target%%#*}")
        [[ "$target" == *"#"* ]] && frag=${target#*#}
        ;;
    esac
    is_changelog_path "$path" || continue
    LINKS=$((LINKS + 1))
    if [ ! -f "$path" ]; then
      fail link-broken "$f -> $target ($path does not exist)"
    elif [ -n "$frag" ] && ! grep -qxF "$path"$'\t'"$frag" "$ANCHORS"; then
      fail anchor-missing "$f -> $target (no section '#$frag' in $path)"
    fi
  done < <(link_targets "$f")
done
echo "ok: $LINKS link(s) into $CHANGELOG or $ARCHIVE_DIR/ checked, across ${#TRACKED[@]} tracked Markdown files"

# 5. Every archive is linked from CHANGELOG.md, at its own path.
CHANGELOG_LINKS=$(link_targets "$CHANGELOG" | while IFS= read -r t; do normalize "${t%%#*}"; done)
for f in ${ARCHIVES[@]+"${ARCHIVES[@]}"}; do
  echo "$CHANGELOG_LINKS" | grep -qxF "$f" || fail archive-unlinked "$f is not linked from $CHANGELOG"
done
[ "${#ARCHIVES[@]}" -gt 0 ] && echo "ok: ${#ARCHIVES[@]} archive file(s) checked for a link from $CHANGELOG"

echo ""
echo "check-changelog result: $ERRORS error(s), $NOTES note(s)"
[ "$ERRORS" -eq 0 ]
