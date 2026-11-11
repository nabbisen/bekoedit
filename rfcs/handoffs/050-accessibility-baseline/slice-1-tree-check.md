# RFC-050 handoff — slice 1: the accessibility check, levels 2 and 3

**Governing RFC:** [RFC-050](../../accepted/RFC-050-accessibility-baseline.md) §5.0, §5.1, §6, §7, §9
**Slice:** 1. Slice 2 (Orca's transcript) follows. Slices 3 and up are fixes.
**Baseline:** `main` at the RFC-050 amendment of 2026-11-11 or later.
**Status:** inherited from RFC-050 (Accepted 2026-10-22, amended 2026-11-11)
**Date:** 2026-10-22, **revised 2026-11-11** for the owner's updated GUI test
rules, `.git-exclude/rules/project-instructions-gui-common.md`. They are binding
on this slice in full. This handoff says how they apply here.
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh` and the task 063 check.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Prove what a screen reader gets from bekoedit, each property at the cheapest
level that can prove it (RFC-050 §5.0):

- **Level 2, headless:** the rendered markup gives every main control an
  accessible name and the right role, in both languages, on every CI run.
- **Level 3, real WebKit through AT-SPI:** that markup reaches the
  accessibility tree, actions work, and one file is saved through
  accessibility actions alone.

Level 1 (translations complete in both languages) already exists, and is
unchanged.

No one has ever looked at this. **Expect findings.** §2.6 says what to do with
them.

## 2. The evidence plan · **[Binding]**

The owner's rule: "Every task states its evidence plan before work starts:
the questions, the level each is checked at, and the captures that remain."
This is that plan. Report against it, and say plainly where you departed from
it and why.

| # | Question | Level | Evidence |
|---|---|---|---|
| Q1 | Does every label key have text in both languages? | 1 | the two existing tests, unchanged |
| Q2 | Does every control in §3.4 render with an accessible name and the right role or ARIA state, in **both** languages and every mode? Is the notices area a live region? | 2 | new headless tests, `dioxus-ssr`, on every CI run |
| Q3 | Does WebKit expose that markup in the AT-SPI tree, with roles and names? | 3 | the observer's verdict; a truncated tree dump on failure |
| Q4 | Can a user open, edit and save a file through accessibility actions alone? | 3 | **the file's bytes on disk**, checked by the app side |
| Q5 | Does a regression in any of these fail by name? | 2 and 3 | §5's mutations |

**Captures that remain: none.** No question here needs a picture. The
evidence is test output, the observer's verdict, the tree dump and the bytes
on disk.

## 3. Required · **[Binding]**

### 3.1 Level 2: headless, both languages

- **`dioxus-ssr`, as published, as a dev-dependency only,** on the line that
  matches our Dioxus 0.7. Never in the shipped binary. Report the full
  `Cargo.lock` diff, and the `cargo tree -e normal` evidence that the release
  build doesn't contain it.
- **Render the shell and each mode,** in English and in Japanese, and assert
  the §3.4 table: each control has an accessible name (its text,
  `aria-label`, or `aria-labelledby` pointing at text). Its role or ARIA state
  is right (`aria-selected` on exactly one tab, for example). The notices
  area carries `aria-live` or `role="alert"`/`"status"` even when empty.
- **The expected names come from `i18n.rs`'s own lookup,** never from strings
  typed into the test. A blank label, or a label in the wrong language, then
  fails.
- **If a component cannot render headless** (for example, it needs the
  desktop window or `eval`), say which one and quote the error. That control
  is then checked at level 3 only. Don't stub it into rendering something
  else.

### 3.2 Level 3: where and how it runs

- **In CI only.** Never on this machine: it starts GTK and WebKit.
- **On the X11 backend, under its own Xvfb,** with the release-check job's
  setup reused, not rebuilt (the owner's "reuse the setup" rule). State the
  backend in the log (`GDK_BACKEND=x11`).
- **In its own D-Bus session,** for example `dbus-run-session`, with the AT-SPI
  bus running and accessibility switched on. Say exactly what switched it on.
- **Packages** go through `scripts/install-linux-build-deps.sh` as extra
  arguments (task 063), for example `at-spi2-core`, `gir1.2-atspi-2.0` and
  `dbus`.
- **When it runs** (RFC-050 §9 Q3, amended): on pushes that change what it
  exercises (`crates/bekoedit-app/**`, `Cargo.toml`, `Cargo.lock`, the check's
  own files and its workflow). The release commit always changes `Cargo.toml`.
  Levels 1 and 2 run on every CI run, as ordinary tests.
- **Japanese,** per RFC-050 §5.1 as amended. English is covered at level 2.

### 3.3 Level 3: isolation, one instance, the observer

- **Isolate every directory the app reads,** the owner's rule. The run sets
  its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and
  `XDG_CACHE_HOME` under the run's scratch directory. **Read from the source
  how each path is resolved, and list them in the request:**
  - settings and recent workspaces through `dirs::config_dir()`
    (`settings.rs`, `recent.rs`);
  - save history and recovery snapshots through `dirs::data_local_dir()`
    (`history.rs`, `recovery.rs`);
  - whatever WebKit keeps, through Dioxus's and `wry`'s data directory.

  Seed the profile as the release-check scenarios do (`seed.rs`), so no
  native dialog is involved: a workspace with `note.md`, one other Markdown
  file and a table, recents, reopen on, language Japanese. **Set the autosave
  delay far beyond the run's timeout,** so only the Save action can save. An
  earlier check here passed through autosave while the button did nothing.
- **One instance.** Zero `bekoedit` processes before the launch, exactly one
  during it. The observer picks **the application whose AT-SPI process id is
  the launched pid,** never the first application named bekoedit.
- **The observer** is a separate process, for example Python with `gi`'s
  `Atspi`, and acts **only through AT-SPI actions:** `do_action`,
  `grab_focus`, `EditableText`. **No keystrokes:** the keyboard is not what
  this slice tests (the owner's rule, "Keystrokes only when the keyboard is
  what is tested"). That includes AT-SPI's own keyboard-event generation, and
  `xdotool`.
- **Assert after every step, and abort at the first anomaly.** Name the step
  and the condition, quote what was found (the nearest roles and names), save
  a truncated tree dump as a CI artifact, and stop. Don't continue to gather
  more.
- **Bounded waits** for named conditions, since the tree fills in
  asynchronously. No polling loops that collect pictures or samples. Wait for
  a condition, with one timeout.

### 3.4 What must be present

The same table for both levels. At level 2, check the markup. At level 3,
find each by **AT-SPI role and exact name.** Report the role WebKit gives each
one, and match on that, not on a guess.

| Area | Must be present |
|---|---|
| Window | the application, and a window whose title names bekoedit (level 3) |
| Explorer | a region named `explorer.label`; one row per seeded file, named by file name, with an action to open it; New file (`explorer.new_file`) |
| Mode tabs | `mode.text`, `mode.form`, `mode.preview` as tabs; **exactly one** selected, the right one |
| Editor | in Text Mode, one editable-text object holding the document. **Report what it is named.** An unnamed editor is a finding (§3.6) |
| Save | the Save control (`editor.save`) with an action; the status readable as one of the `save.*` strings |
| Settings | the control that opens it; the dialog (`settings.title`) with Language (`settings.language`) and Close (`settings.close`); at level 3, opened and closed through actions |
| Form Mode | reached through the Form tab's action at level 3: one named field per editable block; Undo and Redo; the table as a table or grid with its cells |
| Notices | the toast area as a live region, present even when empty |

### 3.5 Level 3: one task through accessibility actions alone

1. Open `note.md` through its explorer row's action. Wait until the editor's
   text is `note.md`'s.
2. Insert `EDIT_MARKER` at the start through the editor's **editable-text
   interface.** If WebKit or CodeMirror doesn't take it, **that is a finding:
   stop and report it** (§3.6), with the editor's text before and after. No
   keyboard fallback.
3. Press Save through its action. Wait for the status to read the `save.saved`
   string.
4. **The app side asserts the exact bytes:** `EDIT_MARKER` followed by the
   original.

### 3.6 What to do with findings

- **A missing name, a wrong role, or a missing live region in our own markup**
  is fixed **in this slice:** an `aria-*` attribute, a role, a label from
  `i18n.rs`, in both languages. Level 2 then holds it. List each fix with the
  test line before and after.
- **Anything larger:** a structural change, focus handling, CodeMirror's own
  DOM, the editable-text finding above, or anything that would weaken a check.
  **Stop and report it with the evidence before landing.** I will decide what
  goes to slice 3 and what slice 1 asserts meanwhile. Never mark an assertion
  "expected to fail", skip it, or loosen it to make the slice green.
- **A problem only level 3 caught, that level 2 could hold** (a name missing in
  the markup, say), gets a level 2 test too, the owner's rule: "so it can't
  come back unseen".

### 3.7 Landing in two steps, and the repeat limit

Level 3 has never run, and only GitHub's runner can run it. A blocking step
that fails on its first run would turn `main` red at the merge. So:

1. **Step A:** levels 2 and 3, with level 3 as a workflow started by hand
   (`workflow_dispatch`) for now. After the merge, one manual run on `main`
   proves it.
2. **Step B, once a manual run is green:** a small follow-up request turns on
   the path-filtered push trigger (§3.2), **blocking** (RFC-050 §9 Q1).

**The repeat limit,** the owner's rule: if the manual run has to be repeated a
second time without a code change, for example to see whether a failure
recurs, stop and ask me before a third. No retries and no
`continue-on-error` in either step. Flakiness is a finding.

### 3.8 Documentation

`docs/src/release-evidence.md`: what each level covers, where level 3's
verdict and tree dump are kept, and how to read a failure. **No CHANGELOG
entry,** unless §3.6's fixes change what a user hears. Then add one line under
`### Fixed`.

## 4. Prohibited · **[Binding]**

- Running GTK, WebKit or the observer on this machine.
- Keystrokes or synthetic input of any kind in the observer.
- Hard-coded expected label strings at either level.
- An "expected to fail", skipped, or loosened assertion; a retry; a longer
  timeout as a fix.
- `dioxus-ssr` outside dev-dependencies. Changing, vendoring or patching
  Dioxus, `wry`, WebKit or CodeMirror.
- Screenshots. None is needed (§2).
- The Flatpak items (RFC-050 §5.4).

## 5. Evidence and review request · **[Binding]**

- **Against the plan (§2),** question by question.
- **Level 2's tests,** with their output, in both languages.
- **Level 3's observer unit tests** on a recorded tree (for example JSON). They
  are headless and run in `lint`. Cover matching by role and name, picking by
  pid, a missing control, a blank name, and the abort report's contents.
- **Mutations, one each, every one caught by name,** at the cheapest level that
  holds it. Level 2's run here. Level 3's run against the recorded tree here,
  and again on the real runner after step A's merge, which I'll name in the
  merge instruction:
  - one control's accessible name removed (level 2, both languages);
  - the Japanese render showing English strings (level 2);
  - the notices area's live attribute removed (level 2);
  - the Save action unreachable, with autosave still far away (level 3);
  - the observer picking an application other than the launched pid (level 3,
    recorded tree).
- **The isolation list** from §3.3, path by path, from the source.
- **§3.6's fixes,** each with before and after.
- **Every local gate,** Rust tests with `CI` unset: `scripts/check-eloc.sh`,
  fmt, clippy 1.88.0 `-D warnings`, the workspace tests, the JS tests,
  `check-changelog`, `check-rfcs`, `check-packaging-versions`, the task 063
  check and its self-test, the other self-tests in `lint`, the full
  `Cargo.lock` diff, and `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-050-slice-1-tree-check.md`.
Give the full tip hash and its base.
