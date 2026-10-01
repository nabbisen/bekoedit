#!/usr/bin/env bash
set -euo pipefail

# Task 042: publishes the six workspace crates to crates.io, called by
# release.yml's publish-crates job after a release is verified and
# published. A real publish needs CARGO_REGISTRY_TOKEN in the environment
# (release.yml gets a short-lived one from crates.io's Trusted Publishing via
# rust-lang/crates-io-auth-action); --dry-run needs none.
#
# Safe to re-run: a crate already published at the exact target version is
# skipped, so a partial failure can be retried without redoing earlier
# crates.
#
# Usage: publish-crates.sh [--dry-run] <version>

usage() {
  echo "usage: publish-crates.sh [--dry-run] <version>" >&2
  exit 1
}

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
  shift
fi

VERSION="${1:-}"
[[ -n "$VERSION" ]] || usage

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Read the declared workspace version straight from Cargo.toml -- not
# `cargo metadata` -- so a mismatch is refused before any cargo call runs at
# all (including the version lookup below, which is itself `cargo info`).
ACTUAL_VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)"[[:space:]]*$/\1/p' Cargo.toml | head -n1)"
if [[ -z "$ACTUAL_VERSION" ]]; then
  echo "publish-crates.sh: could not read [workspace.package] version from Cargo.toml" >&2
  exit 1
fi
if [[ "$ACTUAL_VERSION" != "$VERSION" ]]; then
  echo "publish-crates.sh: workspace is at $ACTUAL_VERSION, not the given $VERSION" >&2
  exit 1
fi

# Dependency order: the four crates bekoedit-core and bekoedit depend on
# (directly or transitively) first, then bekoedit-core, then bekoedit --
# each crate's own dependencies must already resolve on crates.io before it
# is packaged and verified.
CRATES=(
  bekoedit-fs
  bekoedit-markdown
  bekoedit-paste
  bekoedit-ui-contract
  bekoedit-core
  bekoedit
)

for crate in "${CRATES[@]}"; do
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "publish-crates.sh: dry-run $crate@$VERSION"
    cargo publish -p "$crate" --locked --dry-run
    continue
  fi

  if cargo info "$crate@$VERSION" >/dev/null 2>&1; then
    echo "publish-crates.sh: $crate@$VERSION is already on crates.io, skipping"
    continue
  fi

  echo "publish-crates.sh: publishing $crate@$VERSION"
  if ! cargo publish -p "$crate" --locked; then
    echo "publish-crates.sh: publishing $crate failed" >&2
    exit 1
  fi
done

echo "publish-crates.sh: done"
