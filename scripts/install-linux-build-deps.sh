#!/usr/bin/env bash
# Task 063: the one list of system packages that building `bekoedit` on Linux
# needs (the GTK and WebKit development libraries, and libxdo). Every job that
# builds or publishes `bekoedit` on Linux calls this, so the list cannot drift
# between the CI jobs, the release build and the crates.io publish job:
# `cargo publish` verifies a crate by building it, and `bekoedit` needs these
# libraries to build, which the publish job once lacked (the 0.18.0 tag).
# scripts/check-linux-build-deps.py keeps it so, and CI runs that check.
#
# Extra packages a particular job also wants (test tooling, say) are arguments:
#   bash scripts/install-linux-build-deps.sh libssl-dev xvfb
#
# Needs apt and sudo, as the GitHub runners have. Not for a developer machine.

set -euo pipefail

BUILD_DEPS=(libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev)

sudo apt-get update
sudo apt-get install -y "${BUILD_DEPS[@]}" "$@"
