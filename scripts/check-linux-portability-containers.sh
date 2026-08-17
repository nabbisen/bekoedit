#!/usr/bin/env bash
# scripts/check-linux-portability-containers.sh
#
# The only piece of this project's portability tooling allowed to install
# anything, and it only ever does so inside `docker run`, never on the
# host. Pulls a pinned image per distribution, installs the runtime
# dependencies docs/src/distribution.md documents (WebKitGTK 4.1, plus the
# transitive libxdo), then runs check-linux-portability.sh inside the
# container against the mounted binary.
#
# Usage:
#   scripts/check-linux-portability-containers.sh BINARY
#
# Fails loudly -- not silently, and not by falling back to a host-local
# check -- if Docker itself is unavailable. Distinguishes "the image pull
# or package install failed" from "the binary's libraries do not resolve":
# a distribution's mirror being down is not a finding about bekoedit, and
# must not be reported as one.
#
# libxdo.so.3 is expected missing on every distribution below until
# RFC-045 slice 3 lands. Deleting each --expect-missing libxdo.so.3 is
# slice 3's own job, not something to do here ahead of the fix landing --
# the run must fail the moment it no longer needs the exemption, which is
# exactly the coupling that keeps this honest.

set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 BINARY" >&2
  exit 1
fi
BIN="$1"

if [ ! -f "$BIN" ]; then
  echo "Error: binary not found at $BIN" >&2
  exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "Error: docker is not available on this machine. This check only" >&2
  echo "runs inside containers and does not fall back to a host-local" >&2
  echo "check -- installing anything outside a container is out of scope" >&2
  echo "for this tooling (RFC-045 slice 2 SS2.1)." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_ABS="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
INSPECT_SCRIPT="$SCRIPT_DIR/check-linux-portability.sh"

# --- Per-distribution configuration ----------------------------------------
# Images pinned by digest, the same discipline this repository already
# applies to GitHub Actions -- a tag is a moving target, a digest is not.
# Package lists install exactly what docs/src/distribution.md documents as
# the runtime requirement. If a distribution needs more than that to
# resolve everything else, that gap is itself the finding RFC-045 SS5 asks
# this slice to surface -- report it, do not quietly widen the list here.

DISTRO_NAMES=(arch fedora)
DISTRO_IMAGES=(
  "archlinux:base@sha256:b0deabeb3d283da2c7f7dbf0eea051b7b2cd0554e0b737cc457fd21683bdcdd1"
  "fedora:41@sha256:f1a3fab47bcb3c3ddf3135d5ee7ba8b7b25f2e809a47440936212a3a50957f3d"
)
DISTRO_INSTALL=(
  "pacman -Sy --noconfirm --needed webkit2gtk-4.1 xdotool"
  "dnf install -y webkit2gtk4.1 xdotool"
)
# Per RFC-045 SS3: --expect-missing libxdo.so.3 both permits and requires
# it be unresolved on each distribution below. Slice 3 deletes these.
DISTRO_EXPECT_MISSING=(libxdo.so.3 libxdo.so.3)

overall_status=0

for i in "${!DISTRO_NAMES[@]}"; do
  name="${DISTRO_NAMES[$i]}"
  image="${DISTRO_IMAGES[$i]}"
  install_cmd="${DISTRO_INSTALL[$i]}"
  expect_missing="${DISTRO_EXPECT_MISSING[$i]}"

  echo "=== $name ($image) ==="

  if ! docker pull --quiet "$image" >/tmp/bekoedit-pull-"$name".log 2>&1; then
    echo "INFRASTRUCTURE FAILURE: could not pull $image for $name:" >&2
    cat /tmp/bekoedit-pull-"$name".log >&2
    echo "=== $name: INFRASTRUCTURE FAILURE (image pull) -- not a portability finding ===" >&2
    overall_status=1
    continue
  fi

  container_script=/work/check-linux-portability.sh
  container_bin=/work/bekoedit

  # 97 is this script's own sentinel for "install failed inside the
  # container", distinct from the inspection script's 0/1, so the two
  # failure classes never get confused by exit code alone.
  container_cmd="
set -e
$install_cmd >/tmp/install.log 2>&1 || { echo INSTALL_FAILED; cat /tmp/install.log; exit 97; }
echo '--- ldd $container_bin (RFC-045 SS8 evidence) ---'
ldd $container_bin || true
echo '--- libxdo shared object dependency closure (RFC-045 SS5.2 caveat) ---'
libxdo_so=\$(ldconfig -p 2>/dev/null | grep -oE '/[^[:space:]]*libxdo\\.so[^[:space:]]*' | head -1)
if [ -z \"\$libxdo_so\" ]; then
  libxdo_so=\$(find / -xdev -name 'libxdo.so*' 2>/dev/null | head -1)
fi
if [ -n \"\$libxdo_so\" ]; then
  echo \"libxdo shared object: \$libxdo_so\"
  ldd \"\$libxdo_so\" || true
else
  echo 'no libxdo shared object found on this system'
fi
echo '--- portability check ---'
sh $container_script --expect-missing $expect_missing $container_bin
"

  set +e
  output="$(docker run --rm \
    -v "$INSPECT_SCRIPT:$container_script:ro" \
    -v "$BIN_ABS:$container_bin:ro" \
    "$image" \
    sh -c "$container_cmd" 2>&1)"
  status=$?
  set -e

  echo "$output"

  if [ "$status" -eq 97 ]; then
    echo "=== $name: INFRASTRUCTURE FAILURE (package install) -- not a portability finding ==="
    overall_status=1
  elif [ "$status" -ne 0 ]; then
    echo "=== $name: PORTABILITY CHECK FAILED ==="
    overall_status=1
  else
    echo "=== $name: OK ==="
  fi
  echo
done

exit "$overall_status"
