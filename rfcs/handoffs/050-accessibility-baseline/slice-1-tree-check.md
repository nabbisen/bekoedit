# RFC-050 handoff — slice 1: the accessibility-tree check

**Governing RFC:** [RFC-050](../../accepted/RFC-050-accessibility-baseline.md) §5.1, §6, §7, §9
**Slice:** 1. Slice 2 (Orca's transcript) follows. Slices 3 and up are fixes.
**Baseline:** `main` at the RFC-050 acceptance commit or later.
**Status:** inherited from RFC-050 (Accepted 2026-10-22)
**Date:** 2026-10-22
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh` and the task 063 check.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

A blocking CI check that reads bekoedit's accessibility tree the way a screen
reader does, through AT-SPI and nothing else. It checks that the main
controls are there with the right roles and names, in English and in
Japanese, and saves one file through accessibility actions alone.

No one has ever looked at this tree. **Expect findings.** §2.6 says what to do
with them.

## 2. Required · **[Binding]**

### 2.1 Where it runs

- **In CI only,** with the release-check job's setup, under its own Xvfb, and
  **in the end on every push** (RFC-050 §9 Q3), reached in two steps (§2.7).
  Never on this machine: it starts GTK and WebKit.
- **In its own D-Bus session,** for example `dbus-run-session`, with the AT-SPI
  bus running and accessibility switched on. Say in the request exactly what
  switched it on: an environment variable, a setting, or the bus's
  `IsEnabled`.
- **Packages** go through `scripts/install-linux-build-deps.sh` as extra
  arguments (task 063), for example `at-spi2-core`, `gir1.2-atspi-2.0` and
  `dbus`. The task 063 check must still pass.

### 2.2 The app side

- **Seed the profile as the release-check scenarios do** (`seed.rs`), so no
  native dialog is involved: a workspace holding `note.md` and one other
  Markdown file, a recents entry, and reopen on.
- **The language,** English or Japanese, set in the seeded settings.
- **The autosave delay is set far beyond the run's timeout,** so only the Save
  action can save the file. Without this, autosave can make the save step pass
  while the Save button does nothing. An earlier check here passed exactly
  that way.
- **The disk check is made by the app side,** in Rust, as the release checks
  make theirs: the exact expected bytes.
- **The mechanism is [Advisory].** For example, two release-check scenarios,
  `accessibility_tree_en` and `accessibility_tree_ja`. Each seeds, waits for the
  observer's verdict with a bounded timeout, then checks the bytes.

### 2.3 The observer

- **A separate process,** for example Python with `gi`'s `Atspi`, that acts
  **only through AT-SPI.** That means no `xdotool`, no XTEST, no synthetic
  keys or clicks, and no reading the page through the app's own test
  channels. Otherwise it would prove the keyboard path, not the accessibility
  path.
- **The expected names come from the app's own translations, not from the
  observer's code.** For example, the scenario writes the strings it needs,
  from `i18n.rs`'s own lookup, for its language, to a file the observer reads.
  Then a blank label, or a label in the wrong language, fails.
- **Bounded waits.** The tree fills in asynchronously, so wait for named
  conditions. **On a timeout, name the condition and quote what was found:**
  the nearest roles and names, and a truncated dump of the tree, saved as a CI
  artifact.

### 2.4 What must be present

Each item is found by **role and exact name**, in both languages. "Role"
means the AT-SPI role WebKit gives. Report the ones you see, and match on
those; don't hard-code a guess.

| Area | Must be present |
|---|---|
| Window | the application, and a window whose title names bekoedit |
| Explorer | a region named `explorer.label`; one row per seeded file, named by file name, with an action to open it; the New file control (`explorer.new_file`) |
| Mode tabs | `mode.text`, `mode.form`, `mode.preview` as tabs; **exactly one** in the selected state, and it is the right one |
| Editor | in Text Mode, one editable-text object holding the open document's text. **Report what it is named.** An unnamed editor is a finding (§2.6) |
| Save | the Save control (`editor.save`) with an action; the save status readable as one of the `save.*` strings |
| Settings | the control that opens Settings, and through its action, the dialog (`settings.title`) with Language (`settings.language`) and Close (`settings.close`), closed again by Close's action |
| Form Mode | reached through the Form tab's action: one field per editable block in the seeded file, each named; Undo and Redo, the Form history controls; a seeded table exposed as a table or grid, with its cells |
| Notices | the area where toasts appear, exposed as a live region (AT-SPI's `live` or `container-live` attribute, or an alert role), present even when empty |

Add a table to the seeded file if none of the existing seeds has one. Seeds
already used by other scenarios stay as they are.

### 2.5 One task through accessibility actions alone

In that order:

1. Open `note.md` through its explorer row's action. Wait until the editor's
   text is `note.md`'s.
2. Insert `EDIT_MARKER` at the start of the editor's text through AT-SPI's
   **editable-text interface.** If WebKit or CodeMirror doesn't take it,
   don't work around it silently:
   - report what happened, with the editor's text before and after;
   - then use AT-SPI's own keyboard-event generation, which is how on-screen
     keyboards type;
   - say which one worked.
3. Press Save through its action. Wait for the status to read `save.saved`.
4. **The app side asserts the file's exact bytes:** `EDIT_MARKER` followed by
   the original.

### 2.6 What to do with findings

- **A missing name, a wrong role, or a missing live region in our own markup**
  is fixed **in this slice:** an `aria-*` attribute, a role, a label from
  `i18n.rs`, in both languages. List each fix in the request: the element, the
  attribute, and the observer's line before and after.
- **Anything larger:** a structural change, focus handling, anything in
  CodeMirror's own DOM, or anything that would weaken a check. **Stop and
  report it with the evidence before landing.** I will decide whether it goes
  into slice 3 and what slice 1 asserts meanwhile. Never mark an assertion
  "expected to fail", never skip it, and never loosen it to make the slice
  green.

### 2.7 Blocking, in two steps

The check has never run, and only GitHub's runner can run it. A blocking
on-push step that fails on its first run would turn `main` red at the merge.
So it lands as the Arch build did:

1. **Step A:** the check as a workflow started by hand (`workflow_dispatch`
   only), with the same setup as the release-check job. After its merge, a
   manual run on `main` proves it, and a fix round follows if needed.
2. **Step B, once a manual run is green:** a small follow-up request moves it
   into the on-push job, **blocking** (RFC-050 §9 Q1).

No retries, and no `continue-on-error`, in either step. Flakiness is a
finding.

### 2.8 Documentation

`docs/src/release-evidence.md`: a short section on what the check covers,
where its tree dump and verdict are kept, and how to read a failure. **No
CHANGELOG entry,** unless §2.6's fixes change something a user hears. Then
add one line under `### Fixed`, for example: "Screen readers now announce
the … ."

## 3. Prohibited · **[Binding]**

- Running GTK, WebKit or the observer on this machine.
- Synthetic input (`xdotool`, XTEST) in the observer.
- Hard-coding expected label strings in the observer.
- An "expected to fail", skipped, or loosened assertion; a retry; a longer
  timeout as a fix.
- Changing Dioxus, `wry`, WebKit or CodeMirror, or vendoring or patching any
  of them.
- The Flatpak items (RFC-050 §5.4): the app ID and WebKit's sandbox. They
  belong to the Flathub app-side RFC.

## 4. Evidence and review request · **[Binding]**

- **The observer's unit tests,** on a recorded tree (for example JSON). They
  are headless and run in `lint`. Cover matching by role and name, a missing
  control, a blank name, the wrong language, and the timeout report's
  contents.
- **Mutations, one each, every one caught by name,** against the recorded
  tree here. The same four are repeated on the real runner as manual runs
  after step A's merge, and I'll name them in the merge instruction:
  - one label's accessible name removed;
  - the Japanese run reading the English strings;
  - the Save control's action unreachable (with autosave still far away);
  - the notices area's live attribute removed.
- **What you expect at step A's manual run:** both language runs green, their
  timings, and the roles WebKit reported for each control in §2.4.
- **§2.6's fixes,** each with before and after.
- **Every local gate,** Rust tests with `CI` unset: `scripts/check-eloc.sh`,
  fmt, clippy 1.88.0 `-D warnings`, the workspace tests, the JS tests,
  `check-changelog`, `check-rfcs`, `check-packaging-versions`, the task 063
  check and its self-test, the other self-tests in `lint`,
  `git diff --exit-code -- Cargo.lock`, and
  `git diff --stat origin/main...HEAD | tail -1`.

**CI is the first place this runs,** and you cannot push before the review.
So the first real run is step A's manual run after the merge. Expect a
follow-up round for what only the real runner shows. Report what you could
check locally with the recorded tree, and what you could not.

Review request: `.git-exclude/review-request/<date>-rfc-050-slice-1-tree-check.md`.
Give the full tip hash and its base.
