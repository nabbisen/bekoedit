#!/usr/bin/env bash
# Render, build, check, install and start both AUR packages, in a clean Arch
# container, before anything is pushed (RFC-045 slice 3b §2.3). Run by
# .github/workflows/aur.yml as root; builds as an ordinary user, because makepkg
# refuses to run as root. Pushes nothing and holds no key.
#
# Environment:
#   AUR_MODE         "tag"      at a tag: nothing is placed in advance; makepkg
#                               downloads the tag's source tarball and the
#                               PUBLISHED Arch archive and checks them against the
#                               rendered sums, which is the check that those sums
#                               are what users will download;
#                    "dry-run"  a manual run: the source package uses the latest
#                               existing tag's tarball, and the Arch archive for
#                               bekoedit-bin is the one in AUR_ARCH_ARCHIVE (built
#                               by slice 3a's workflow, since none is published
#                               before the first release that carries one). It is
#                               placed in the build directory under its source
#                               name, where makepkg uses it and still checks its sum.
#   AUR_ARCH_ARCHIVE the path of that archive (dry-run only)
#   AUR_OUT          where to write <pkgname>/PKGBUILD and <pkgname>/.SRCINFO
#   GITHUB_REPOSITORY, GITHUB_REF_NAME   as GitHub sets them
#
# The pacman installs below are the proof environment (what a user's machine has:
# base-devel, the repository's own `rust`), deliberately NOT the release build
# environment of scripts/install-arch-build-deps.sh, which uses rustup. That is
# why scripts/check-linux-build-deps.py names this file as the second place a
# raw pacman install is allowed.

set -euo pipefail

: "${AUR_MODE:?AUR_MODE must be tag or dry-run}"
: "${AUR_OUT:?AUR_OUT must be set}"
: "${GITHUB_REPOSITORY:?}"
case "$AUR_MODE" in
  tag) VERSION="${GITHUB_REF_NAME:?}" ;;
  dry-run)
    : "${AUR_ARCH_ARCHIVE:?AUR_ARCH_ARCHIVE must be set in a dry run}"
    VERSION="$(awk '/^\[workspace\.package\]/{in_section=1; next} /^\[/{in_section=0} in_section && /^version[[:space:]]*=/{gsub(/[^0-9.]/, ""); print; exit}' Cargo.toml)"
    ;;
  *) echo "AUR_MODE must be tag or dry-run, not '$AUR_MODE'" >&2; exit 1 ;;
esac
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid bare SemVer version: $VERSION" >&2
  exit 1
fi

REPO_DIR="$(pwd)"
BUILDER=builder
HOME_DIR="/home/$BUILDER"
banner() { printf '\n==== %s ====\n' "$*"; }
as_builder() { runuser -u "$BUILDER" -- env HOME="$HOME_DIR" "$@"; }

TEMPLATES=("packaging/linux/bin/PKGBUILD" "packaging/linux/PKGBUILD")
PACKAGES=("bekoedit-bin" "bekoedit")

banner "mode $AUR_MODE, version $VERSION"

banner "the proof environment: a full upgrade, then what a user has"
pacman -Syu --noconfirm --needed base-devel namcap rust python curl
id "$BUILDER" >/dev/null 2>&1 || useradd -m "$BUILDER"
install -d -o "$BUILDER" -g "$BUILDER" "$HOME_DIR/repo" "$HOME_DIR/render" "$HOME_DIR/build" "$HOME_DIR/files"
cp -r "$REPO_DIR/scripts" "$REPO_DIR/packaging" "$HOME_DIR/repo/"
chown -R "$BUILDER:$BUILDER" "$HOME_DIR/repo"
mkdir -p "$AUR_OUT"

# The files a template's `source` array points to, fetched into <dir> under their
# local names, in source order, printed one path per line. In a dry run the Arch
# archive is the locally built one instead of a download. The template is sourced
# (it is repository code, as scripts/check-packaging-versions.sh also does).
fetch_sources() { # <template> <dir>
  local template="$1" dir="$2"
  mkdir -p "$dir"
  (
    # shellcheck disable=SC1090
    source "$template"
    for entry in "${source[@]}"; do
      name="${entry%%::*}"
      url="${entry#*::}"
      if [ "$AUR_MODE" = dry-run ] && [[ "$url" == *-unknown-linux-gnu-arch.tar.gz ]]; then
        cp "$AUR_ARCH_ARCHIVE" "$dir/$name"
      else
        curl -fsSL --retry 3 -o "$dir/$name" "$url"
      fi
      echo "$dir/$name"
    done
  )
}

package_deps() { # <srcinfo>: depends and makedepends, version operators stripped
  sed -n 's/^[[:space:]]*\(make\)\{0,1\}depends = //p' "$1" | sed 's/[<>=].*//' | sort -u
}

check_installed() { # the installed binary and files, for the package just installed
  local needed
  needed="$(readelf -d /usr/bin/bekoedit | grep NEEDED)"
  printf '%s\n' "$needed"
  printf '%s\n' "$needed" | grep -qF 'libxdo.so.4' \
    || { echo "the installed binary does not list libxdo.so.4" >&2; return 1; }
  if printf '%s\n' "$needed" | grep -qF 'libxdo.so.3'; then
    echo "the installed binary lists libxdo.so.3" >&2
    return 1
  fi
  sh "$REPO_DIR/scripts/check-linux-portability.sh" /usr/bin/bekoedit
  env -u DISPLAY -u WAYLAND_DISPLAY timeout 10 /usr/bin/bekoedit --headless-smoke
  echo "--headless-smoke exit code: $?"
  test -f /usr/share/applications/bekoedit.desktop
  test -f /usr/share/pixmaps/bekoedit.png
  echo "the .desktop file and the icon are in place"
}

for index in 0 1; do
  template="${TEMPLATES[$index]}"
  name="${PACKAGES[$index]}"
  render_dir="$HOME_DIR/render/$name"
  build_dir="$HOME_DIR/build/$name"

  banner "$name: fetch what the template's sources point to, and render"
  mapfile -t files < <(fetch_sources "$HOME_DIR/repo/$template" "$HOME_DIR/files/$name")
  chown -R "$BUILDER:$BUILDER" "$HOME_DIR/files"
  as_builder python "$HOME_DIR/repo/scripts/aur-render.py" "$VERSION" "$HOME_DIR/repo/$template" "$render_dir" "${files[@]}"
  cat "$render_dir/PKGBUILD"
  cat "$render_dir/.SRCINFO"

  banner "$name: dependencies"
  mapfile -t deps < <(package_deps "$render_dir/.SRCINFO")
  printf 'declared: %s\n' "${deps[*]}"
  # Only what is not already satisfied (a virtual name such as `cargo` is met by
  # the installed `rust`); pacman -T prints the unmet ones and exits 127 for them.
  mapfile -t missing < <(pacman -T "${deps[@]}" || true)
  printf 'to install: %s\n' "${missing[*]:-nothing}"
  if [ "${#missing[@]}" -gt 0 ]; then
    pacman -S --noconfirm --needed "${missing[@]}"
  fi

  banner "$name: makepkg, from a clean build directory"
  rm -rf "$build_dir"
  install -d -o "$BUILDER" -g "$BUILDER" "$build_dir"
  cp "$render_dir/PKGBUILD" "$build_dir/PKGBUILD"
  chown "$BUILDER:$BUILDER" "$build_dir/PKGBUILD"
  if [ "$AUR_MODE" = dry-run ] && [ "$name" = bekoedit-bin ]; then
    # Placed under its source name: makepkg uses it, and still checks its sum.
    archive_local="$(basename "${files[0]}")"
    cp "$AUR_ARCH_ARCHIVE" "$build_dir/$archive_local"
    chown "$BUILDER:$BUILDER" "$build_dir/$archive_local"
    echo "dry run: placed the locally built Arch archive as $archive_local"
  fi
  (cd "$build_dir" && as_builder makepkg --noconfirm --force)

  banner "$name: namcap on the PKGBUILD and the built package (an error fails; warnings are printed)"
  namcap_log="$HOME_DIR/namcap-$name.log"
  (
    cd "$build_dir"
    namcap PKGBUILD
    for built in ./*.pkg.tar.*; do namcap "$built"; done
  ) 2>&1 | tee "$namcap_log"
  # Every built file is checked, the -debug package included. One error is excused,
  # by shape: that package's build-id symlinks point into the main package, which
  # namcap cannot see (scripts/aur-namcap-errors.py says exactly which).
  if ! python "$REPO_DIR/scripts/aur-namcap-errors.py" < "$namcap_log"; then
    echo "namcap reported errors for $name" >&2
    exit 1
  fi

  banner "$name: install exactly this package, and check the installed files"
  # Arch's default makepkg.conf also builds a $name-debug package. It is namcap'd
  # above with the rest, but never installed: it would outlive the `pacman -R`
  # below and conflict with the next package's own -debug file.
  mapfile -t built_files < <(printf '%s\n' "$build_dir"/*.pkg.tar.*)
  printf 'built: %s\n' "${built_files[*]}"
  package_file="$(python "$REPO_DIR/scripts/aur-pick-package.py" "$name" "${built_files[@]}")"
  echo "installing: $package_file"
  pacman -U --noconfirm "$package_file"
  check_installed
  pacman -R --noconfirm "$name"

  mkdir -p "$AUR_OUT/$name"
  cp "$render_dir/PKGBUILD" "$render_dir/.SRCINFO" "$AUR_OUT/$name/"
done

banner "both packages were rendered, built, checked, installed and started"
ls -l "$AUR_OUT"/*
