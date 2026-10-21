# RFC-050: Accessibility baseline: what a screen reader gets

**Project:** bekoedit
**Status:** Proposed — drafted 2026-10-22, after the owner accepted the
direction the same day: a CI accessibility check; the Flatpak fixes recorded
for the Flathub app-side RFC; a real screen reader run by the dev team rather
than by the owner. §9 asks the remaining questions.
**Track:** Accessibility
**Priority:** Medium — never verified on any platform; a known Flatpak defect
in the same stack
**Date:** 2026-10-22
**Related RFCs:** [RFC-044](../done/RFC-044-shell-behaviour-regression-coverage.md),
[RFC-048](../done/RFC-048-form-mode-table-editing.md)
**Reference:** another project on the same stack (WebKitGTK 4.1 under
`wry`), `.git-exclude/tmp/flatpak-screen-reader-issue-20261021.md`

---

## 1. Summary

Find out, and keep finding out at every change, what a screen reader gets from
bekoedit. There are two checks in CI, both on Linux:

1. **The accessibility tree.** Are the controls there, with names and roles,
   and can a file be edited and saved through accessibility actions alone?
2. **A real screen reader's speech.** Orca, run headless, records what it
   would say while the main controls are reached by keyboard. The transcript
   is an artifact a person can read.

Then fix what they find.

## 2. Motivation

**Nobody has ever checked.** bekoedit draws its whole interface inside a web
view. The platform's web view exposes that page to the screen-reader
interface: AT-SPI on Linux, UI Automation on Windows, the Accessibility API on
macOS. What a screen reader says depends on our markup. There is some:

- `aria-selected`, `aria-expanded`, `aria-disabled`, `aria-label`,
  `aria-hidden` and `aria-labelledby`, across the Rust components;
- task 044 fixed seven labels that had been blank since 0.11.0;
- RFC-048 slice 5 covered the table grid's keyboard use and accessibility.

But no one has listened to it with Orca, NVDA or VoiceOver, and no CI check
looks at the accessibility tree. The honest status is "probably partly works,
unverified".

**A project on the same stack found a defect** (2026-10-21). Under Flatpak,
its web content was invisible to assistive technology until two things were
fixed:

- WebKit's own sandbox was turned on;
- the app ran under its Flatpak app ID.

WebKit's bug tracker explains the cause. Inside Flatpak, the web process's
accessibility bus name derives from the app ID, and only a web process that
`flatpak-spawn` sandboxed may own it
([WebKit bug 325394](https://bugs.webkit.org/show_bug.cgi?id=325394)).
bekoedit has both gaps:

- `wry` 0.53.5 never turns WebKit's sandbox on (no `sandbox` anywhere in its
  `webkitgtk` backend);
- bekoedit sets no app ID.

That matters for the Flathub plan (§5.4). It does not affect today's
downloads.

## 3. Goals

- A blocking CI check that the main controls are in the accessibility tree,
  with roles and names, in both UI languages.
- One real task done through accessibility actions alone, proved on disk.
- A recorded transcript of what Orca says, kept with each run.
- Every gap the checks find either fixed, or written down with a reason.
- The Flatpak conditions recorded where the Flatpak work will pick them up.

## 4. Non-goals

- A full WCAG audit. This is a baseline, not a certification.
- Windows and macOS checks in this RFC (§9 Q2).
- CodeMirror's own internals. We check that the editor is reachable, named and
  editable; how CodeMirror 6 voices its content is upstream's.
- Any change to Dioxus, `wry` or WebKit. Our fixes are in our own markup and
  setup. Upstream crates are used as published: nothing vendored, nothing patched.

## 5. Design

### 5.1 The accessibility-tree check (Linux)

It runs **beside** the release-check scenarios, under the same Xvfb, in a
D-Bus session with the AT-SPI bus running. bekoedit runs normally. A separate
observer process reads the tree through AT-SPI (Python's `gi` bindings for
`Atspi`) and acts only through it. It sends no synthetic keys and no clicks:
those would prove the keyboard path, not the accessibility path.

**The controls that must be present, each with a role and a non-empty name:**

| Area | What must be exposed |
|---|---|
| Window | the application, with its window title |
| Workspace explorer | the region named "Workspace explorer"; each file row, named by its file name, selectable; New file, Rename, Delete |
| Mode tabs | Text, Form, Preview, as tabs with a selected state |
| The editor | an editable text object, named, holding the document text |
| Save state | the save status ("All changes saved", "Unsaved changes" …) readable as text |
| Form Mode | each field named after its block; table cells as a grid with row and column; Undo and Redo |
| Settings | the dialog and its controls |
| Notices | toasts exposed so a screen reader announces them, for example as a live region or alert |

The exact list is fixed in the slice handoff, from what the app has at the
time.

**One task through accessibility actions alone.** Select `note.md` through
its explorer row's action. Insert text through the editor's editable-text
interface. Press Save through its action. **Assert the bytes on disk.** The
other project proved exactly this, and it shows the tree is usable, not just
present.

**Both languages.** The check runs once in English and once in Japanese, so a
missing translation that leaves a label blank fails it. Task 044 found seven
of those by hand.

**Blocking from the start** (§9 Q1), like every other step in CI. If it is
flaky, the flakiness is a finding to fix, not a reason to soften it. No
retries.

### 5.2 Orca's transcript (Linux)

Orca is the screen reader Linux users actually use. Run it headless in the
same session, with its speech written to a log instead of a synthesiser. The
mechanism is the slice's to find, for example Orca's debug output, or a
speech-dispatcher output module that writes text.

Then reach the main controls by keyboard, the way a screen-reader user
would: Tab through the shell, switch modes, move through the explorer, and
enter the editor.

- **Asserted:** every focus stop produces speech that includes a name. No stop
  is announced as a bare role ("button", "push button", "tab") or as nothing.
- **Kept:** the full transcript, as a CI artifact. A person can read it to
  judge whether it makes sense, which no assertion can.

**Feasibility is the slice's first question.** If Orca cannot run reliably
headless on GitHub's runner, the slice says so with evidence. §5.1 stands
alone, and the transcript becomes a dev-team task in a container instead.

### 5.3 Fixes

Whatever §5.1 and §5.2 find becomes ordinary tasks, in our own markup:
missing names, wrong roles, missing live regions, focus that lands nowhere.
RFC-044's focus finding (Ctrl+2 then Ctrl+1 leaves nothing focused) is likely
to show up here as a focus stop with no speech. If it does, it is fixed under
this RFC.

### 5.4 Under Flatpak: recorded for the Flathub app-side RFC

Not built here, because there is no Flatpak yet. Recorded so the Flathub work
starts from it:

- **The app ID:** set the GTK application ID to the Flatpak app ID. Dioxus
  accepts our own event loop (`Config::with_event_loop`), and `tao`'s Unix
  event-loop builder takes `with_app_id`. **Only inside Flatpak, until
  checked:** a GTK application with an ID is single-instance by default, so a
  second launch may hand over to the first window instead of opening its own.
- **WebKit's sandbox:** on, inside Flatpak only. `wry` creates its own web
  context, so the API on WebKit's default context does not reach it. WebKit's
  `WEBKIT_FORCE_SANDBOX` switch does
  ([background](https://blogs.gnome.org/mcatanzaro/2020/03/31/sandboxing-webkitgtk-apps/)),
  and must be set before WebKit starts. **Never outside Flatpak:** there the
  sandbox needs `bubblewrap`, and without it WebKit cannot start a web
  process.
- **The proof** is §5.1's check run inside the Flatpak test build. It is the
  same check, so a Flatpak regression shows as the same named failure.

## 6. Testing

- The checks are themselves the tests. Each one must be **shown to fail:**
  - a control's label removed;
  - a toast's live region removed;
  - the Save action unreachable.

  Each must fail by name.
- The observer's tree-walking and matching code gets unit tests on a recorded
  tree, with no display.
- Nothing here runs GTK, WebKit or Orca on the owner's machine. CI, or a
  container, only.

## 7. Risks

- **Flakiness:** the accessibility tree is filled in asynchronously. The
  observer must wait for named conditions with a bounded timeout and report
  what it saw, as the release checks do.
- **Orca under Xvfb** may not be dependable (§5.2). There is a fallback, and
  §5.1 does not depend on it.
- **Findings may be many.** A first look at an unaudited interface usually
  finds more than expected. §5.3 takes them one task at a time, highest
  impact first.

## 8. Slices

| # | Content |
|---|---|
| 1 | §5.1: the tree check, both languages, the accessibility-only save; blocking |
| 2 | §5.2: Orca's transcript, asserted and kept; feasibility first |
| 3+ | §5.3: fixes, one task each |

§5.4 goes to the Flathub app-side RFC, not here.

## 9. Questions for the owner

**Q1. Blocking from the start?** *Proposed: yes,* like every other CI step
since the trusted-click promotion. A flaky accessibility check is a defect to
fix, not a check to soften.

**Q2. Windows and macOS.** *Proposed: later, as a separate slice or RFC.*
GitHub's Windows runner can read UI Automation, which is how WebView2 exposes
the page, and macOS can read its Accessibility API. Both are possible, but
each is a project of its own. Linux comes first because it has the known
defect, and because Orca runs in CI.

**Q3. Where it runs.** *Proposed: on every push,* like the release-check
scenarios, because a label can break in any change. About two more minutes per
run for §5.1. §5.2's cost is measured in its slice.
