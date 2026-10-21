#!/usr/bin/env bash
# Push one proven AUR package (RFC-045 slice 3b): PKGBUILD and .SRCINFO from
# PACKAGE_DIR into the AUR repository REMOTE, only if they are newer.
#
# Usage:  AUR_GIT_NAME=... AUR_GIT_EMAIL=... scripts/aur-push.sh PACKAGE_DIR REMOTE
#
# PACKAGE_DIR holds the two files CI proved, and nothing is re-rendered here.
# REMOTE is the AUR repository (ssh://aur@aur.archlinux.org/<pkgname>.git in CI,
# a local bare repository in the tests). SSH is configured by the caller through
# GIT_SSH_COMMAND, which git reads itself; this script never touches a key.
#
# It clones REMOTE and compares `pkgver-pkgrel` in the new .SRCINFO with the one
# in the AUR's current .SRCINFO:
#   newer     -> copy in the two files, commit "<pkgname> <pkgver>-<pkgrel>", and
#                push to master (never --force; a remote that moved is a normal
#                non-fast-forward failure);
#   equal, and both files identical -> "already up to date", success;
#   equal but different, or older   -> fail, naming both versions. This is what
#                stops a re-run of an old tag from pushing an older version, or
#                overwriting a packaging fix made by hand.
# A repository with no .SRCINFO yet counts as older than anything. Any other file
# in the repository is left alone, and listed in the log. Nothing is ever deleted.
#
# The comparison is plain numeric on bare SemVer `N.N.N` plus an integer pkgrel,
# which is exactly what Arch's vercmp gives for those (scripts/test-aur-push.sh
# checks the two against each other where vercmp is installed). Anything else on
# either side -- an epoch, a pkgver that is not N.N.N, a non-integer pkgrel -- is
# refused rather than guessed at, since vercmp's rules for them are not these.

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 PACKAGE_DIR REMOTE" >&2
  exit 2
fi
PACKAGE_DIR="$1"
REMOTE="$2"
: "${AUR_GIT_NAME:?AUR_GIT_NAME is not set (the commit identity comes from the environment)}"
: "${AUR_GIT_EMAIL:?AUR_GIT_EMAIL is not set (the commit identity comes from the environment)}"

for file in PKGBUILD .SRCINFO; do
  if [ ! -f "$PACKAGE_DIR/$file" ]; then
    echo "aur-push: refused: $PACKAGE_DIR/$file is missing" >&2
    exit 1
  fi
done

srcinfo_value() { # <file> <key>: the first `key = value` line
  sed -n "s/^[[:space:]]*$2 = //p" "$1" | head -n1
}

is_semver() { [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }
is_integer() { [[ "$1" =~ ^[0-9]+$ ]]; }

# compare_pair <a.b.c> <x.y.z>: prints -1, 0 or 1. 10# avoids octal readings.
compare_pair() {
  local IFS=. left right i
  read -ra left <<<"$1"
  read -ra right <<<"$2"
  for i in 0 1 2; do
    if ((10#${left[i]} > 10#${right[i]})); then echo 1; return; fi
    if ((10#${left[i]} < 10#${right[i]})); then echo -1; return; fi
  done
  echo 0
}

# compare_versions <ver> <rel> <ver> <rel>
compare_versions() {
  local by_version
  by_version="$(compare_pair "$1" "$3")"
  if [ "$by_version" != "0" ]; then echo "$by_version"; return; fi
  if (("10#$2" > "10#$4")); then echo 1; elif (("10#$2" < "10#$4")); then echo -1; else echo 0; fi
}

NEW_SRCINFO="$PACKAGE_DIR/.SRCINFO"
PKGNAME="$(srcinfo_value "$NEW_SRCINFO" pkgbase)"
NEW_VER="$(srcinfo_value "$NEW_SRCINFO" pkgver)"
NEW_REL="$(srcinfo_value "$NEW_SRCINFO" pkgrel)"
if [ -z "$PKGNAME" ] || ! is_semver "$NEW_VER" || ! is_integer "$NEW_REL" \
  || grep -q '^[[:space:]]*epoch = ' "$NEW_SRCINFO"; then
  echo "aur-push: refused: the new .SRCINFO does not name a pkgbase, a bare SemVer pkgver and an integer pkgrel (or has an epoch): '${PKGNAME}' '${NEW_VER}' '${NEW_REL}'" >&2
  exit 1
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/aur-push.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
git clone --quiet "$REMOTE" "$WORK/repo"
cd "$WORK/repo"
# The AUR takes pushes to master only.
if git rev-parse --verify --quiet HEAD >/dev/null; then
  git checkout --quiet master
else
  git checkout --quiet -B master
fi

others="$(git ls-files | grep -vxF -e PKGBUILD -e .SRCINFO || true)"
if [ -n "$others" ]; then
  echo "aur-push: $PKGNAME: left alone in the AUR repository: $(echo "$others" | paste -sd' ' -)"
fi

if [ -f .SRCINFO ]; then
  OLD_VER="$(srcinfo_value .SRCINFO pkgver)"
  OLD_REL="$(srcinfo_value .SRCINFO pkgrel)"
  if ! is_semver "$OLD_VER" || ! is_integer "$OLD_REL" || grep -q '^[[:space:]]*epoch = ' .SRCINFO; then
    echo "aur-push: refused: the AUR's current .SRCINFO has pkgver '$OLD_VER' and pkgrel '$OLD_REL' (or an epoch), which this script does not compare; leaving $PKGNAME alone" >&2
    exit 1
  fi
  ORDER="$(compare_versions "$NEW_VER" "$NEW_REL" "$OLD_VER" "$OLD_REL")"
else
  OLD_VER="(none)"
  OLD_REL=""
  ORDER=1
fi

case "$ORDER" in
  0)
    if cmp -s "$PACKAGE_DIR/PKGBUILD" PKGBUILD && cmp -s "$PACKAGE_DIR/.SRCINFO" .SRCINFO; then
      echo "aur-push: $PKGNAME $NEW_VER-$NEW_REL: already up to date"
      exit 0
    fi
    echo "aur-push: refused: $PKGNAME $NEW_VER-$NEW_REL is already on the AUR, with different PKGBUILD or .SRCINFO; not overwriting (the AUR has $OLD_VER-$OLD_REL, this run has $NEW_VER-$NEW_REL)" >&2
    exit 1
    ;;
  -1)
    echo "aur-push: refused: this run has $PKGNAME $NEW_VER-$NEW_REL, which is older than the AUR's $OLD_VER-$OLD_REL; not pushing an older version" >&2
    exit 1
    ;;
esac

cp "$PACKAGE_DIR/PKGBUILD" PKGBUILD
cp "$PACKAGE_DIR/.SRCINFO" .SRCINFO
git add PKGBUILD .SRCINFO
git -c user.name="$AUR_GIT_NAME" -c user.email="$AUR_GIT_EMAIL" commit --quiet -m "$PKGNAME $NEW_VER-$NEW_REL"
echo "aur-push: $PKGNAME: $OLD_VER${OLD_REL:+-$OLD_REL} -> $NEW_VER-$NEW_REL, pushing to master"
git push origin HEAD:master
echo "aur-push: $PKGNAME $NEW_VER-$NEW_REL pushed"
