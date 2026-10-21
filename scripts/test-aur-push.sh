#!/usr/bin/env bash
# Self-test for scripts/aur-push.sh (RFC-045 slice 3b), against local bare
# repositories. Nothing here touches the AUR, a key, or the network.
# Run from the repo root: bash scripts/test-aur-push.sh
set -u
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PUSH="$SCRIPT_DIR/aur-push.sh"
FAILURES=0
CASES=0
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

export AUR_GIT_NAME="CI Account"
export AUR_GIT_EMAIL="ci@example.invalid"
# A clean git environment: none of the caller's global or system config.
export GIT_CONFIG_GLOBAL="$WORK/gitconfig"
export GIT_CONFIG_SYSTEM=/dev/null
: >"$GIT_CONFIG_GLOBAL"
git config --global user.name fixture
git config --global user.email fixture@example.invalid
git config --global init.defaultBranch master
git config --global advice.detachedHead false

ok() { CASES=$((CASES + 1)); echo "ok:   $1"; }
fail() { CASES=$((CASES + 1)); FAILURES=$((FAILURES + 1)); echo "FAIL: $1"; shift; printf '        %s\n' "$@"; }

# srcinfo <file> <pkgname> <ver> <rel>
srcinfo() {
  printf 'pkgbase = %s\n\tpkgdesc = fixture\n\tpkgver = %s\n\tpkgrel = %s\n\tarch = x86_64\n\npkgname = %s\n' \
    "$2" "$3" "$4" "$2" >"$1"
}

# package <case> <pkgname> <ver> <rel> [extra line for PKGBUILD]: in a directory named
# after the package, as the job's aur-out/<package> is
package() {
  local dir="$WORK/pkg-$1/$2"
  mkdir -p "$dir"
  printf 'pkgname=%s\npkgver=%s\npkgrel=%s\n%s\n' "$2" "$3" "$4" "${5:-}" >"$dir/PKGBUILD"
  srcinfo "$dir/.SRCINFO" "$2" "$3" "$4"
  echo "$dir"
}

# remote <name> <pkgname> <ver> <rel>: a bare repository holding that version, like the
# owner's placeholder. With no version: an empty bare repository.
remote() {
  local bare="$WORK/remote-$1/$2.git" seed="$WORK/seed-$1"
  mkdir -p "$WORK/remote-$1"
  git init --quiet --bare "$bare"
  git -C "$bare" config receive.denyNonFastForwards true
  if [ "$#" -ge 4 ]; then
    git clone --quiet "$bare" "$seed" 2>/dev/null
    printf 'pkgname=%s\npkgver=%s\n' "$2" "$3" >"$seed/PKGBUILD"
    srcinfo "$seed/.SRCINFO" "$2" "$3" "$4"
    git -C "$seed" add PKGBUILD .SRCINFO
    git -C "$seed" commit --quiet -m placeholder
    git -C "$seed" push --quiet origin HEAD:master
  fi
  echo "$bare"
}

commits() { git -C "$1" rev-list --count master 2>/dev/null || echo 0; }
head_of() { git -C "$1" rev-parse master 2>/dev/null || echo none; }

run() { OUT="$("$PUSH" "$@" 2>&1)"; STATUS=$?; }

# --- 1. the first push over the owner's 0.0.0-1 placeholder ---
r=$(remote first bekoedit 0.0.0 1); p=$(package first bekoedit 0.18.1 1 "# the real one")
run "$p" "$r"
if [ "$STATUS" -eq 0 ] && [ "$(commits "$r")" = 2 ] \
  && [ "$(git -C "$r" log -1 --format=%s master)" = "bekoedit 0.18.1-1" ] \
  && [ "$(git -C "$r" log -1 --format='%an <%ae>' master)" = "CI Account <ci@example.invalid>" ] \
  && git -C "$r" show master:PKGBUILD | cmp -s - "$p/PKGBUILD" \
  && git -C "$r" show master:.SRCINFO | cmp -s - "$p/.SRCINFO"; then
  ok "the first push over a 0.0.0-1 placeholder: one new commit named 'bekoedit 0.18.1-1', the identity from the environment, both files exact"
else fail "the first push over a placeholder" "status $STATUS" "$OUT"; fi

# --- 2. a re-run with identical files: up to date, no new commit ---
before="$(head_of "$r")"
run "$p" "$r"
if [ "$STATUS" -eq 0 ] && [ "$(head_of "$r")" = "$before" ] && [ "$(commits "$r")" = 2 ] \
  && echo "$OUT" | grep -q "already up to date"; then
  ok "a re-run with identical files is 'already up to date' and makes no commit"
else fail "a re-run with identical files" "status $STATUS" "$OUT"; fi

# --- 3. an older version is refused, naming both ---
old=$(package older bekoedit 0.18.0 1)
before="$(head_of "$r")"
run "$old" "$r"
if [ "$STATUS" -ne 0 ] && [ "$(head_of "$r")" = "$before" ] \
  && echo "$OUT" | grep -q "0.18.0-1" && echo "$OUT" | grep -q "0.18.1-1" && echo "$OUT" | grep -q "older"; then
  ok "an older version is refused, naming both versions, and the remote is unchanged"
else fail "an older version" "status $STATUS" "$OUT"; fi

# --- 4. the same version with different files is refused ---
diff=$(package differs bekoedit 0.18.1 1 "# a hand edit would be overwritten")
run "$diff" "$r"
if [ "$STATUS" -ne 0 ] && [ "$(head_of "$r")" = "$before" ] && echo "$OUT" | grep -q "already on the AUR, with different"; then
  ok "the same version with different files is refused, and the remote is unchanged"
else fail "the same version, different files" "status $STATUS" "$OUT"; fi

# --- 5. a higher pkgrel, and a higher pkgver, are newer ---
rel2=$(package rel2 bekoedit 0.18.1 2)
run "$rel2" "$r"
if [ "$STATUS" -eq 0 ] && [ "$(git -C "$r" log -1 --format=%s master)" = "bekoedit 0.18.1-2" ]; then ok "0.18.1-2 is newer than 0.18.1-1"
else fail "a higher pkgrel" "status $STATUS" "$OUT"; fi
v10=$(package v10 bekoedit 0.18.10 1)
run "$v10" "$r"
if [ "$STATUS" -eq 0 ] && [ "$(git -C "$r" log -1 --format=%s master)" = "bekoedit 0.18.10-1" ]; then ok "0.18.10 is newer than 0.18.1 (numeric, not text, order)"
else fail "0.18.10 over 0.18.1" "status $STATUS" "$OUT"; fi
run "$(package v9 bekoedit 0.18.9 1)" "$r"
if [ "$STATUS" -ne 0 ]; then ok "0.18.9 is older than 0.18.10 and is refused"
else fail "0.18.9 over 0.18.10" "status $STATUS" "$OUT"; fi

# --- 6. the remote moved during the run: a non-fast-forward failure, no force ---
# The remote moves after the script has cloned it and before it pushes (a post-checkout
# hook in the script's clone, enabled through the environment, does it once). The remote
# does NOT deny non-fast-forwards here, so only the script's own refusal to force stands
# between its commit and the owner's.
r2=$(remote moved bekoedit 0.0.0 1); p2=$(package moved bekoedit 0.18.1 1)
git -C "$r2" config receive.denyNonFastForwards false
hooks="$WORK/hooks"; mkdir -p "$hooks"
cat >"$hooks/post-checkout" <<'HOOK'
#!/bin/sh
# While the script is under way, the owner pushes a fix to the AUR repository (once).
[ -e "$MOVE_MARK" ] && exit 0
: >"$MOVE_MARK"
unset GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0 GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX
tmp="$(mktemp -d)"
git clone -q "$MOVE_REMOTE" "$tmp/c"
cd "$tmp/c" && echo hand-made > fix.txt && git add fix.txt && git commit -qm "owner's fix" && git push -q origin HEAD:master
rm -rf "$tmp"
HOOK
chmod +x "$hooks/post-checkout"
OUT="$(MOVE_MARK="$WORK/moved.mark" MOVE_REMOTE="$r2" GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0="$hooks" "$PUSH" "$p2" "$r2" 2>&1)"; STATUS=$?
if [ "$STATUS" -ne 0 ] && [ -e "$WORK/moved.mark" ] && [ "$(git -C "$r2" log -1 --format=%s master)" = "owner's fix" ] \
  && ! git -C "$r2" log --format=%s master | grep -q "bekoedit 0.18.1-1"; then
  ok "a remote that moved during the run fails the push, and the owner's commit is still the head (no force)"
else fail "a remote that moved" "status $STATUS" "$OUT" "head: $(git -C "$r2" log -1 --format=%s master)"; fi

# --- 7. an extra file in the remote is left alone and listed ---
r3=$(remote extra bekoedit 0.0.0 1)
seed="$WORK/seed-extra"
echo "owner's note" >"$seed/NOTES.txt"; git -C "$seed" add NOTES.txt; git -C "$seed" commit --quiet -m notes; git -C "$seed" push --quiet origin HEAD:master
run "$(package extra bekoedit 0.18.1 1)" "$r3"
if [ "$STATUS" -eq 0 ] && git -C "$r3" show master:NOTES.txt | grep -q "owner's note" \
  && echo "$OUT" | grep -q "left alone in the AUR repository: NOTES.txt"; then
  ok "an extra file in the remote is left alone, and listed in the log"
else fail "an extra file in the remote" "status $STATUS" "$OUT"; fi

# --- 8. a brand-new, empty AUR repository ---
r4=$(remote empty bekoedit-bin)
run "$(package empty bekoedit-bin 0.18.1 1)" "$r4"
if [ "$STATUS" -eq 0 ] && [ "$(git -C "$r4" log -1 --format=%s master)" = "bekoedit-bin 0.18.1-1" ]; then ok "an empty remote takes the first commit on master"
else fail "an empty remote" "status $STATUS" "$OUT"; fi

# --- 9. refusals before anything is cloned ---
run "$WORK/nonexistent" "$r"
if [ "$STATUS" -ne 0 ] && echo "$OUT" | grep -q "is missing"; then ok "a package directory without the files is refused"; else fail "missing files" "$OUT"; fi
OUT="$(env -u AUR_GIT_NAME "$PUSH" "$p" "$r" 2>&1)"; STATUS=$?
if [ "$STATUS" -ne 0 ] && echo "$OUT" | grep -q "AUR_GIT_NAME"; then ok "no commit identity in the environment is refused (none is hard-coded)"; else fail "no identity" "$OUT"; fi
bad=$(package epoch bekoedit 0.19.0 1); printf '\tepoch = 1\n' >>"$bad/.SRCINFO"
run "$bad" "$r"
if [ "$STATUS" -ne 0 ] && echo "$OUT" | grep -q "epoch"; then ok "a new .SRCINFO with an epoch is refused"; else fail "epoch" "$OUT"; fi
bad=$(package dev bekoedit 0.19.0.r5 1)
run "$bad" "$r"
if [ "$STATUS" -ne 0 ] && echo "$OUT" | grep -q "bare SemVer"; then ok "a pkgver that is not N.N.N is refused"; else fail "non-SemVer pkgver" "$OUT"; fi
r5=$(remote odd bekoedit 0.18.1.r5 1)
before="$(head_of "$r5")"
run "$(package odd bekoedit 0.19.0 1)" "$r5"
if [ "$STATUS" -ne 0 ] && [ "$(head_of "$r5")" = "$before" ] && echo "$OUT" | grep -q "does not compare"; then ok "an AUR .SRCINFO this script cannot compare is refused, not guessed at"
else fail "uncomparable remote" "$OUT"; fi

# --- 9b. the package must be the repository's ---
good=$(package match bekoedit 0.18.1 1); rm_=$(remote match bekoedit 0.0.0 1)
wrongdir="$WORK/wrong-dir/bekoedit-bin"; mkdir -p "$WORK/wrong-dir"; cp -r "$good" "$wrongdir"
before="$(head_of "$rm_")"
run "$wrongdir" "$rm_"
if [ "$STATUS" -ne 0 ] && [ "$(head_of "$rm_")" = "$before" ] && echo "$OUT" | grep -q "not the repository's" \
  && echo "$OUT" | grep -q "package directory is 'bekoedit-bin'"; then
  ok "a package directory whose name is not the pkgbase is refused, naming all three, before any commit"
else fail "wrong directory name" "status $STATUS" "$OUT"; fi
wrongremote=$(remote mismatch bekoedit-bin 0.0.0 1)
before="$(head_of "$wrongremote")"
run "$good" "$wrongremote"
if [ "$STATUS" -ne 0 ] && [ "$(head_of "$wrongremote")" = "$before" ] && echo "$OUT" | grep -q "the repository is 'bekoedit-bin'"; then
  ok "a pkgbase pushed to another package's repository is refused, and that repository is unchanged"
else fail "wrong remote" "status $STATUS" "$OUT"; fi
run "$good" "$rm_/"
if [ "$STATUS" -eq 0 ] && [ "$(git -C "$rm_" log -1 --format=%s master)" = "bekoedit 0.18.1-1" ]; then
  ok "a remote written with a trailing slash still names its repository"
else fail "trailing slash" "status $STATUS" "$OUT"; fi

# --- 10. the script never forces ---
code="$(grep -v '^[[:space:]]*#' "$PUSH")"
pushes="$(echo "$code" | grep -c 'git push' || true)"
if [ "$pushes" = 1 ] && echo "$code" | grep -qx 'git push origin HEAD:master' \
  && ! echo "$code" | grep -qE -e '--force|--delete|--mirror|git push -f|git push [^|]*[[:space:]]\+'; then
  ok "the script's only push is 'git push origin HEAD:master', with no force, mirror or delete"
else fail "force flags" "$(echo "$code" | grep 'git push')"; fi

# --- 11. the comparison agrees with Arch's vercmp ---
if command -v vercmp >/dev/null 2>&1; then
  bad=0
  for a in 0.0.0-1 0.18.0-1 0.18.1-1 0.18.1-2 0.18.9-1 0.18.10-1 0.19.0-1 1.0.0-1 0.9.0-3 0.10.0-1; do
    for b in 0.0.0-1 0.18.0-1 0.18.1-1 0.18.1-2 0.18.9-1 0.18.10-1 0.19.0-1 1.0.0-1 0.9.0-3 0.10.0-1; do
      expect="$(vercmp "$a" "$b")"
      # shellcheck disable=SC1090
      got="$(source <(sed -n '/^compare_pair()/,/^}/p;/^compare_versions()/,/^}/p' "$PUSH"); compare_versions "${a%-*}" "${a#*-}" "${b%-*}" "${b#*-}")"
      [ "$expect" = "$got" ] || { bad=1; echo "        $a vs $b: vercmp $expect, script $got"; }
    done
  done
  if [ "$bad" -eq 0 ]; then ok "the comparison gives vercmp's answer for every pair of 10 versions (100 pairs)"; else fail "vercmp agreement"; fi
else
  echo "skip: vercmp agreement (vercmp is not installed on this machine)"
fi

echo ""
echo "test-aur-push: $CASES case(s), $FAILURES failure(s)"
[ "$FAILURES" -eq 0 ]
