#!/usr/bin/env python3
"""Owns the X CLIPBOARD selection with BOTH a text/html and a text/plain flavour, the way a browser
or an office app does, until it is killed. For the `paste_probe` release-checks scenario (RFC-046
slice 2, part A), which runs it in CI under Xvfb and nowhere else.

Usage: webview-clipboard-owner.py <html> <plain> <ready-file>

Why not `xclip`: it serves ONE target per process (`-t text/html`), and a second xclip takes the
selection away from the first, so it cannot offer both flavours at once. GTK's clipboard can.

Why GTK 4, not GTK 3 (part A2, 2026-09-30): GTK 3's `Gtk.Clipboard.set_with_data` takes a get/clear
callback pair that PyGObject cannot bind -- it is not introspectable -- so the first version of this
script (GTK 3) failed at run time with `AttributeError: 'Clipboard' object has no attribute
'set_with_data'` (task 026 run 36210132615). GTK 4's `Gdk.Clipboard.set_content`, with a
`Gdk.ContentProvider`, is plain data and needs no callback, so it works from Python. The CI package
is `gir1.2-gtk-4.0`.

Only the three byte providers are used: `text/html`, `text/plain;charset=utf-8` and `text/plain`. A
fourth provider was tried, wrapping the plain text as a `GLib.Variant("s", plain)` for a `G_TYPE_VARIANT`
`GValue`, on the reasoning that it would cover any target GTK maps from a `GValue` rather than raw bytes.
It was removed on review, before it ever ran: the two byte providers already offer `text/plain` and
`text/plain;charset=utf-8`, and GTK maps those to the X plain-text targets (`UTF8_STRING`, `STRING`)
itself, so the variant provider added nothing -- and it was also the one line most likely to fail on its
first run, since GTK has no standard serialiser for a bare `G_TYPE_VARIANT` value. A `GValue` provider
for completeness would instead be `new_for_value(plain)` with a plain Python `str`, which PyGObject
turns into a `G_TYPE_STRING`; it is not required either.

It writes `READY` to <ready-file> once it owns the selection. It needs a display (DISPLAY, which
xvfb-run sets), so it must never be run on a developer's desktop: it would take over the real
clipboard.
"""
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk  # noqa: E402

html, plain, ready_file = sys.argv[1], sys.argv[2], sys.argv[3]

# GTK 4 needs a display connection, which only exists once the toolkit is
# initialised; `Gtk.init()` does that without opening a window.
Gtk.init()

providers = [
    Gdk.ContentProvider.new_for_bytes("text/html", GLib.Bytes.new(html.encode("utf-8"))),
    Gdk.ContentProvider.new_for_bytes(
        "text/plain;charset=utf-8", GLib.Bytes.new(plain.encode("utf-8"))
    ),
    Gdk.ContentProvider.new_for_bytes("text/plain", GLib.Bytes.new(plain.encode("utf-8"))),
]
provider = Gdk.ContentProvider.new_union(providers)

display = Gdk.Display.get_default()
if display is None:
    sys.stderr.write("no default display (DISPLAY unset, or no X server)\n")
    sys.exit(2)
clipboard = display.get_clipboard()
if not clipboard.set_content(provider):
    sys.stderr.write("could not take the clipboard\n")
    sys.exit(2)

with open(ready_file, "w") as handle:
    handle.write("READY\n")

GLib.MainLoop().run()
