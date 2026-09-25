#!/usr/bin/env bash
# Task 033: stub OS openers for the `link_clicks_reach_only_the_browser`
# release-checks scenario. Usage: webview-link-opener-stubs.sh <dir>
#
# Creates, in <dir>:
#   opener.log    an empty log, the only place any stub writes
#   browser-stub  for $BROWSER. `webbrowser` (Linux) runs $BROWSER first, with
#                 the URL as its argument, and stops there if it starts. It
#                 appends that argument, bare, as one line.
#   <name>        one shim for every other opener `webbrowser` 1.2.4 can reach
#                 on Linux (unix.rs: xdg-settings, then gio, gvfs-open,
#                 gnome-open, kde-open, kde-open5, kfmclient, exo-open,
#                 mate-open, xdg-open, x-www-browser; sensible-browser as well).
#                 Each appends `UNEXPECTED-OPENER <name> <args>` and exits 1.
#
# The caller runs the app with BROWSER=<dir>/browser-stub,
# PATH=<dir>:$PATH and BEKOEDIT_LINK_OPENER_LOG=<dir>/opener.log. With that,
# the log holds every URL that reached an opener, and any opener other than
# $BROWSER shows up as an UNEXPECTED-OPENER line instead of running.
set -euo pipefail

dir="${1:?usage: $0 <dir>}"
mkdir -p "$dir"
dir="$(cd "$dir" && pwd)"
log="$dir/opener.log"
: > "$log"

cat > "$dir/browser-stub" <<EOF
#!/bin/sh
printf '%s\n' "\$1" >> '$log'
exit 0
EOF
chmod +x "$dir/browser-stub"

for name in xdg-settings gio gvfs-open gnome-open kde-open kde-open5 kfmclient \
            exo-open mate-open xdg-open x-www-browser sensible-browser; do
  cat > "$dir/$name" <<EOF
#!/bin/sh
printf 'UNEXPECTED-OPENER %s %s\n' '$name' "\$*" >> '$log'
exit 1
EOF
  chmod +x "$dir/$name"
done

printf '%s\n' "$dir"
