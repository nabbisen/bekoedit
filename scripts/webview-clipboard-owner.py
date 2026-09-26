#!/usr/bin/env python3
"""Owns the X CLIPBOARD selection with BOTH a text/html and a text/plain flavour, the way a browser
or an office app does, until it is killed. For the `paste_probe` release-checks scenario (RFC-046
slice 2, part A), which runs it in CI under Xvfb and nowhere else.

Usage: webview-clipboard-owner.py <html> <plain> <ready-file>

Why not `xclip`: it serves ONE target per process (`-t text/html`), and a second xclip takes the
selection away from the first, so it cannot offer both flavours at once. GTK's clipboard can.

It writes `READY` to <ready-file> once it owns the selection. It needs a display (DISPLAY, which
xvfb-run sets), so it must never be run on a developer's desktop: it would take over the real
clipboard.
"""
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

html, plain, ready_file = sys.argv[1], sys.argv[2], sys.argv[3]

HTML_TARGET = "text/html"
TEXT_TARGETS = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"]


def provide(_clipboard, selection, info, _data):
    if info == 0:
        selection.set(Gdk.Atom.intern(HTML_TARGET, False), 8, html.encode("utf-8"))
    else:
        selection.set_text(plain, -1)


def released(_clipboard, _data):
    pass


clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
targets = [Gtk.TargetEntry.new(HTML_TARGET, 0, 0)] + [Gtk.TargetEntry.new(t, 0, 1) for t in TEXT_TARGETS]
if not clipboard.set_with_data(targets, provide, released, None):
    sys.stderr.write("could not take the clipboard\n")
    sys.exit(2)
with open(ready_file, "w") as handle:
    handle.write("READY\n")
Gtk.main()
