#!/bin/sh
# scripts/check-linux-portability.sh
#
# Pure inspection: does every shared library a Linux binary needs resolve
# on the system running this script? Installs nothing, calls nothing but
# `ldd`, writes nothing. Safe to run by hand on any machine, including
# this project's own Arch-family reference system (RFC-045 §8) -- it is
# only meaningful if it is reproducible outside CI, and this project has
# already been burned by a check nobody could run locally.
#
# Usage:
#   scripts/check-linux-portability.sh [--expect-missing NAME ...] BINARY
#
# Each --expect-missing NAME both PERMITS and REQUIRES that library to be
# unresolved here:
#   - unresolved, not in --expect-missing  -> fail, naming it.
#   - unresolved, in --expect-missing      -> permitted, no failure.
#   - in --expect-missing but it resolves  -> fail, naming it as a STALE
#     expectation. This is the anti-rot half: an exemption that stops
#     being true must be noticed, not carried forever.
#
# This does NOT fix anything, install anything, or launch the binary.
# A container that wants these libraries present first has to install
# them itself, before calling this script -- see
# scripts/check-linux-portability-containers.sh.

set -eu

expect_missing=""
bin=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --expect-missing)
      if [ "$#" -lt 2 ]; then
        echo "Error: --expect-missing requires a library name" >&2
        exit 1
      fi
      expect_missing="$expect_missing $2"
      shift 2
      ;;
    -*)
      echo "Error: unknown option: $1" >&2
      echo "Usage: $0 [--expect-missing NAME ...] BINARY" >&2
      exit 1
      ;;
    *)
      break
      ;;
  esac
done

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 [--expect-missing NAME ...] BINARY" >&2
  exit 1
fi
bin="$1"

if [ ! -f "$bin" ]; then
  echo "Error: binary not found at $bin" >&2
  exit 1
fi

if ! command -v ldd >/dev/null 2>&1; then
  echo "Error: ldd not found on this system." >&2
  exit 1
fi

unresolved="$(ldd "$bin" 2>&1 | grep 'not found' | sed -E 's/^[[:space:]]*([^[:space:]]+).*/\1/' || true)"

fail=0

for lib in $unresolved; do
  permitted=0
  for expected in $expect_missing; do
    if [ "$lib" = "$expected" ]; then
      permitted=1
      break
    fi
  done
  if [ "$permitted" -eq 1 ]; then
    echo "expected missing (permitted): $lib"
  else
    echo "UNRESOLVED (not permitted): $lib"
    fail=1
  fi
done

for expected in $expect_missing; do
  still_unresolved=0
  for lib in $unresolved; do
    if [ "$lib" = "$expected" ]; then
      still_unresolved=1
      break
    fi
  done
  if [ "$still_unresolved" -eq 0 ]; then
    echo "STALE EXPECTATION (library resolves): $expected"
    fail=1
  fi
done

if [ "$fail" -eq 0 ]; then
  echo "All libraries required by $bin resolve as expected on this system."
  exit 0
fi

exit 1
