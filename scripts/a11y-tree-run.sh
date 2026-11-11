#!/usr/bin/env bash
# RFC-050 slice 1, level 3: run bekoedit's `a11y_tree` release-checks scenario and
# the AT-SPI observer (scripts/a11y-observer.py) side by side, in CI only.
#
#   bash scripts/a11y-tree-run.sh PATH/TO/bekoedit OUT_DIR
#
# Backend: X11, on Xvfb (GDK_BACKEND=x11). The accessibility bus is a private
# session D-Bus (dbus-run-session) with at-spi2-core's bus launcher; it is switched
# on the way a desktop session switches it on, by setting org.a11y.Status IsEnabled
# over the session bus before the app starts, and the app is started with
# NO_AT_BRIDGE=0 (the other release-checks scenarios start it with 1).
#
# Isolation: HOME, every XDG_* directory, TMPDIR and the runtime directory are under
# one scratch directory; the app's own state is in its isolated profile. One
# instance: the script refuses to start if a bekoedit process already runs, and the
# observer selects the application by the launched process id.
#
# It refuses to run outside GitHub Actions: nothing that starts GTK, WebKit or
# a window runs on a developer machine, and a live accessibility bus there would
# be the real desktop's.
set -euo pipefail

if [ "${1:-}" = "--inside" ]; then
  shift
  BINARY="$1"; OUT="$2"; SCRATCH="$3"
  PROFILE="$SCRATCH/bekoedit-webview-smoke-profile"

  echo "a11y-tree: display backend X11 on Xvfb ($DISPLAY); accessibility bus: private session bus + at-spi-bus-launcher"
  /usr/libexec/at-spi-bus-launcher --launch-immediately &
  BUS=$!
  trap 'kill "$BUS" 2>/dev/null || true' EXIT
  dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set \
    string:org.a11y.Status string:IsEnabled variant:boolean:true >/dev/null
  echo "a11y-tree: org.a11y.Status IsEnabled = $(dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus org.freedesktop.DBus.Properties.Get string:org.a11y.Status string:IsEnabled | tail -1 | tr -s ' ')"

  mkdir -p "$OUT"
  NO_AT_BRIDGE=0 GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
    "$BINARY" --webview-release-checks "$PROFILE" a11y_tree &
  APP=$!
  echo "a11y-tree: launched bekoedit, pid $APP"

  observer=0
  python3 scripts/a11y-observer.py --pid "$APP" --profile "$PROFILE" --out "$OUT" || observer=$?
  app=0
  wait "$APP" || app=$?
  echo "a11y-tree: observer exit $observer, app exit $app"
  [ "$observer" -eq 0 ] && [ "$app" -eq 0 ]
  exit
fi

if [ "${GITHUB_ACTIONS:-}" != "true" ]; then
  echo "a11y-tree-run: refused: this runs only in GitHub Actions (it starts a real window and an accessibility bus)" >&2
  exit 2
fi
BINARY="$(realpath "${1:?usage: a11y-tree-run.sh PATH/TO/bekoedit OUT_DIR}")"
OUT="$(realpath -m "${2:?usage: a11y-tree-run.sh PATH/TO/bekoedit OUT_DIR}")"
if pgrep -x bekoedit >/dev/null; then
  echo "a11y-tree-run: refused: a bekoedit process is already running (one instance only)" >&2
  exit 2
fi

SCRATCH="$(mktemp -d "${RUNNER_TEMP:-/tmp}/bekoedit-a11y.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
mkdir -p "$SCRATCH"/{home,config,data,cache,state,run,tmp}
chmod 700 "$SCRATCH/run"

env \
  HOME="$SCRATCH/home" XDG_CONFIG_HOME="$SCRATCH/config" XDG_DATA_HOME="$SCRATCH/data" \
  XDG_CACHE_HOME="$SCRATCH/cache" XDG_STATE_HOME="$SCRATCH/state" XDG_RUNTIME_DIR="$SCRATCH/run" \
  TMPDIR="$SCRATCH/tmp" LC_ALL=C.UTF-8 \
  xvfb-run -a -s "-screen 0 1280x1024x24" \
  dbus-run-session -- bash "$0" --inside "$BINARY" "$OUT" "$SCRATCH"
