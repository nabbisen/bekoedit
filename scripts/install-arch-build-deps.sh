#!/usr/bin/env bash
# RFC-045 slice 3a: the one list of Arch Linux packages that building `bekoedit`
# needs, as scripts/install-linux-build-deps.sh is for apt (task 063). Every job
# that builds `bekoedit` in an Arch container calls this and not the apt script,
# so the list cannot drift. scripts/check-linux-build-deps.py keeps it so, and
# CI runs that check.
#
# It starts with a full `pacman -Syu`: Arch is rolling, and installing packages
# into an out-of-date system (a partial upgrade) is unsupported. Extra packages
# a particular job also wants are arguments:
#   bash scripts/install-arch-build-deps.sh some-package
#
# Needs pacman and root, as inside the pinned archlinux container. Not for a
# developer machine.
#
# `rustup` provides the pinned compiler: Arch's own `rust` package is whatever is
# current, not 1.88.0, so the job installs that toolchain itself.

set -euo pipefail

BUILD_DEPS=(base-devel pkgconf git rustup webkit2gtk-4.1 gtk3 xdotool openssl)

pacman -Syu --noconfirm --needed "${BUILD_DEPS[@]}" "$@"
