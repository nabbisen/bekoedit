# RFC-044 handoff — slice 3: mode tabs, focus authority, Settings and Recovery, conflict banner

**Governing RFC:** [RFC-044](../../accepted/RFC-044-shell-behaviour-regression-coverage.md) §8 C–F, §10, §11
**Slice:** 3 of 3
**Baseline:** `main` at `472b13f` or later, with slice 2 merged. If `main` has moved
in a file this slice touches, merge `origin/main` in; never rebase. Read the run's
state rather than assuming it. When slice 2's terminal phase finishes,
`sub/child.md` is open in **Form** mode and focus is on a tree row.
**Status:** inherited from RFC-044 (Accepted 2026-08-24)
**Date:** 2026-09-17

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding sections are mine to
decide. For an advisory mechanism, you may replace it with your own reasoning
without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`.

## 1. Purpose, and why this slice is staged

C–F are four contract families. RFC-044 §14 Q3 keeps them in one slice. They
differ sharply in what they need from the harness:

| Family | Needs |
|---|---|
| C. Mode tabs | existing machinery |
| D. Focus authority | existing machinery |
| E. Settings and Recovery | a Recovery snapshot seeded at launch; two identifying attributes (§6) |
| F. Conflict banner | a Rust-side file write between two phases; an autosave fixture setting |

One merge keeps the promotion clock to one restart. Splitting it into two slices
would restart the clock twice for the same coverage. But carrying the new harness
capabilities in the same step as C and D would make a failure harder to locate,
which is the reason RFC-044 split A from B.

So the slice has **two stages and an interim review between them** (§12). Stage 1
is C and D. Stage 2 is E and F. There is still one branch and one merge.

## 2. What already exists · context, verified on `472b13f`

**Mode tabs** (`components/editor_header/mode_tabs.rs`)

- A `nav` with `role="tablist"` and `id="editor-mode-switch"`, containing three
  tabs in this DOM order: Text (`data-source-focus-launch="mode-text"`), Preview
  (`mode-preview`) and Form (`mode-form`).
- Split is **not** a tab. It lives in the tools menu.
- The selected tab carries `tabindex="0"`, `aria-selected="true"` and the class
  `active`. The other tabs carry `tabindex="-1"`.
- The tablist's `onkeydown` uses `tab_key_intent`: Right → Next, Left → Previous,
  Home → First, End → Last. It calls `focus_tab`, which moves DOM focus and
  **nothing else**. It is DOM-relative and wraps through the shared
  `focus_move_expr`.
- **Enter and Space are not handled by the tablist.** The code comment says they
  "still reach each tab's own `onclick` via native button-activation semantics".
  That makes them a browser default action. §3.3 explains why that matters.
- Each tab's `onclick` submits `SourceCommand::SwitchMode` through
  `submit_source_interaction`. Switching into Text claims editor focus
  (`focus_target`).

**Focus-leave close** (`app.rs`, the `onfocusin` of `app-frame`)

- It calls `release_menu_focus` and sets `open_menu` to `None`, **without**
  restoring focus. This is the path slice 2's contract 7 covered, with a tree row
  as the outside element.

**Settings** (`components/settings_screen.rs`)

- Entered from the **last** item of the app menu. That item has no identifying
  attribute (`app_bar.rs:241-272`). Its handler keeps shell authority held.
- `role="region"`, `aria-labelledby="settings-heading"`. The `h1` has
  `id="settings-heading"` and `tabindex="-1"`, and receives focus in a
  `use_effect` on entry.
- The footer has **Save** (`class="primary"`) and **Close**. Save writes the
  settings file and sets the mode to the configured default. Close only closes.
  Neither control has an id.
- On exit, `close_settings` releases authority, calls
  `focus_element(TRIGGER_APP_MENU)`, and then sets `settings_open` to false.

**Correction to my own earlier note.** Task 017's finding review §6 called the
Settings exit restore "the strongest suspect", on the grounds that the trigger
might not exist when the focus lands. That reasoning is wrong. `AppBar` is
rendered **outside** the `if settings_open` switch in `app.rs`, so the app-menu
trigger stays mounted throughout Settings, and the restore target always exists.

The risk that is real is different. When Settings closes, `MainShell` mounts
again. In Text mode that mounts the source editor. If anything then claims focus,
it lands **after** the restore and takes focus away from the trigger. The same
applies when Recovery exits and restores to the logo. E2 and E4 test exactly
that.

**Recovery** (`components/recovery_screen.rs`)

- Shown when `should_show_recovery` is true: recovery was pending at launch, it
  has not been dismissed, and **no session is open**. It sits before `MainShell`
  in `app.rs`'s switch, so it covers a reopened workspace.
- `role="region"`, `aria-labelledby="recovery-heading"`. The `h2` has
  `id="recovery-heading"` and `tabindex="-1"`, and receives focus in a
  `use_effect`, which also acquires shell authority.
- A `role="status"` paragraph reads "`{count} {suffix}`".
- The actions are per-snapshot **Restore** and **Discard**, plus **Skip all**
  (`class="btn-ghost recovery-skip"`). Skip all removes every snapshot and
  dismisses the screen.
- `use_drop` releases authority and calls `focus_element(TRIGGER_APP_LOGO)`.

**Conflict banner** (`components/conflict_banner.rs`, rendered in `MainShell`
under `EditorHeader`)

- Rendered only when `conflict.requires_user_decision()`, which means
  `DiskChangedDirtyMemory` or `FileDeletedOnDisk`. `DiskChangedCleanMemory`
  renders nothing.
- `role="alert"`. Three buttons: **Keep my version**, **Reload from disk**, and
  **Save my version as a copy**. Reload is absent when the file was deleted, so
  three buttons means dirty memory, not deletion.
- The banner has no focus handling at all, which is what RFC-042 §7.6 requires.

**The poll** (`app.rs`): every `TICK_MS` (500 ms), when a session is open,
`check_external_change()` runs and then `autosave_tick`.

**Detection** (`bekoedit-fs/src/atomic.rs`): the file counts as changed if its
**length or FNV-1a content hash** changed. mtime alone does not count.

**Autosave** (`bekoedit-core/src/save.rs`): due at `edit time +
autosave_debounce_ms`. Saving a dirty document makes it clean. Seeded settings can
hold any value; the Settings screen's 300–10 000 clamp applies only to what a user
types.

**Closing the run with a dirty document** is safe. No close handler, message
dialog or unload prompt exists anywhere in `bekoedit-app/src`, which I checked by
grep. RFC-041's run already ends dirty.

## 3. The contracts · **[Binding]**

### C. Mode tabs

Starting state: a document open in **Form**.

**C1. Arrow movement is focus only.** `.focus()` the selected tab, which is Form.
Then, one key at a time: **Right** (wraps to Text), **Left** (back to Form),
**Home** (Text), **End** (Form). After each key:

- `document.activeElement` is the expected tab;
- **the Form tab is still the only tab with `aria-selected="true"`**, and it still
  holds `tabindex="0"`;
- that stays true for the whole observation window (§5). A tab that activates on
  arrow keys would change the mode after the focus move, not before it.

Check after **every** key, not once at the end. Right then Left starts and ends on
Form, so a single check at the end would pass a build that switched the mode twice.

**C2. Activation, through bekoedit's half.** C1 ends with focus on the Form tab.
`.click()` the Text tab (§3.3 explains why this is not Enter). A script `click()`
runs the handler but does not move focus, so the focus asserted below comes from
the claim, not from the click. Assert:

- Text is now the only selected tab, and it holds `tabindex="0"`;
- the source editor is mounted and **has focus**, because `SwitchMode(Text)`
  claims it.

Find the Text tab by `[data-source-focus-launch="mode-text"]`. Require exactly one
match, as task 016's Form tab did.

### §3.3 Enter and Space on a tab are the A.1 trap, a third time

The tablist handles only arrows, Home and End. Enter and Space activate a tab
because the browser's **native** button activation turns them into a click. That
is a default action, and a synthetic `KeyboardEvent` gets no default action. A
synthetic Enter would do nothing, and C2 would time out on a build that works
perfectly.

So C2 drives bekoedit's half: `.click()` runs the tab's `onclick`, which is the
code RFC-042 §7.3's manual activation depends on. Whether a *real* Enter produces
that click is browser behaviour. It is out of scope for the same reason as A.1.

RFC-044 §11 says there are **two** exceptions to the "dispatch a key, assert
focus" recipe. This is a third. I am correcting §11 in the same commit as this
handoff.

### D. Focus authority

Starting state: Text mode, editor focused (after C2).

**D1. Focus entering the editor closes the menu and stays.** Open the app menu by
keyboard (Down on the trigger; the first item is focused). Then call **the editor
view's own `focus()`** (`window.__bk._view.focus()`). Assert, using slice 2's
`focusLeavesMenu` with the editor's content element as the outside element:

- the menu container is gone, and the trigger reads `aria-expanded="false"`;
- `document.activeElement` is inside the editor, and `view.hasFocus` is true;
- that holds for the observation window. There is no restore to the trigger, and
  nothing else takes focus.

**D2. The close released shell authority.** A menu that closes without releasing
authority looks correct to D1 and breaks every later focus claim. That is the
failure task 016 found with New File. Prove the release by making a claim
afterwards:

- `.click()` the Preview tab, then the Text tab;
- assert the editor **takes focus** again.

`SwitchMode(Text)` claims editor focus, and a claim made while authority is still
held is refused.

### E. Settings and Recovery

**E1. Recovery entry** (the first phase of the run, §4.1).

- The Recovery region is present.
- `document.activeElement` is `#recovery-heading`.
- The `role="status"` element's text begins with `1 `.

**E2. Recovery exit.** `.click()` **Skip all** (`.recovery-skip`, exactly one
match). Assert:

- the Recovery region is gone and the tree is present;
- `document.activeElement` is `#app-bar-logo-trigger`, for the whole observation
  window.

Then the run continues into slice 1's `down_up`, which now also proves Recovery
released shell authority: task 014's `enter_opens` claim would otherwise be
refused.

**E3. Settings entry.** Starting state after D2: Text mode, editor focused.

1. Open the app menu by keyboard.
2. `.click()` the Settings item through its identifying attribute (§6), and only
   after the guard in §6 passes.
3. Assert the Settings region is present and `document.activeElement` is
   `#settings-heading`.

**E4. Settings exit.** `.click()` the **Close** button through its identifying
attribute (§6). Assert:

- the Settings region is gone and `MainShell` is back (the Text-mode editor host
  is present);
- `document.activeElement` is `#app-menu-trigger`, **for the whole observation
  window**.

The window is the point of E4 (§2's correction). A focus restore that the editor's
remount then overrides is what E4 exists to catch.

### F. Conflict banner

Starting state after E4: Text mode, `child.md` open, focus on the app-menu
trigger.

**F1. Dirty the document.**

1. Focus the editor (`view.focus()`) and dispatch one insertion into the
   CodeMirror view, the same way RFC-041's `edit_dispatched` does.
2. Wait for the header's `.dirty-dot`, which reflects `session.dirty` in Rust.
3. Assert `.file-name` reads `child.md`, so the Rust write in §4.2 targets the
   open document.

**F2. The banner appears and focus does not move.** The Rust sequence writes the
file between F1 and F2 (§4.2). In F2:

1. Record `document.activeElement`. It must be inside the editor; fail with its
   name if it is not.
2. Wait for `.conflict-banner[role="alert"]`.
3. Assert it has **exactly three** buttons. That means `DiskChangedDirtyMemory`:
   the deleted-file banner has two, and clean memory renders no banner at all.
4. For the observation window, `document.activeElement` must remain the recorded
   element and must never be inside the banner.

F2 is the terminal phase.

## 4. Harness additions · **[Binding] on the property, [Advisory] on the shape**

### 4.1 Recovery is seeded at launch, and its phases come first

Recovery appears only at launch, only while no session is open, and before
`MainShell`. It cannot be reached mid-run. So:

- `prepare()` saves **one** `RecoverySnapshot` into the isolated profile's
  recovery directory before launch. Its `original_path` is `workspace/a.md`, with
  text that differs from the file. Use `RecoveryStore::save`, the same API
  `AppState` uses. Do not hand-write the file format.
- `recovery_entry` and `recovery_exit` become the **first two phases**, before
  `down_up`.

**No third run mode.** RFC-044 §5 keeps a second run, and a phase at the start of
that run costs nothing a third launch would not.

**Only Skip all is ever clicked.** Restore opens a session and changes the state
of every later phase. Discard leaves the question of a remaining snapshot open.

### 4.2 A Rust-side write between F1 and F2

The driver cannot write files, and the harness must not reach through the bridge
to do it. The Rust sequence (`run_shell_behaviour_sequence`) already runs between
phases:

- After F1 reports progress, and **before** F2 is requested, overwrite
  `workspace/sub/child.md` with content of a **different length**, since detection
  is length plus hash (§2).
- Hand the workspace path from `prepare()` to the sequence. Do not rediscover it at
  run time.
- If the write fails, the run fails, naming the path and the error.

Shape (a per-phase "before" hook, a match arm, or something else) is advisory.
Keep it to one place, with a comment naming F.

### 4.3 Autosave must not clean the document first

With the default 1 500 ms debounce, the document autosaves before the Rust write.
The write then yields `DiskChangedCleanMemory`, and no banner is rendered.
RFC-044 §8 F says so explicitly: a clean document is a different and harmless
state.

- **Seed `core.autosave_debounce_ms = 86_400_000`** (one day) in `prepare()`'s
  settings, and comment why.
- Not `u64::MAX`: `note_edit` adds the debounce to the current time, and that sum
  must not overflow.
- No earlier phase depends on autosave. Untitled documents cannot autosave, and no
  phase saves.

### 4.4 Phase list

In this order:

1. `recovery_entry`
2. `recovery_exit`
3. slice 1's, task 014's, task 016's and slice 2's phases, unchanged
4. `tabs_arrows_focus_only`
5. `tabs_click_activates`
6. `menu_closes_into_editor`
7. `authority_released_after_editor_focus`
8. `settings_entry`
9. `settings_exit_restored`
10. `conflict_dirtied`
11. `conflict_banner_focus_kept` (terminal)

Names are advisory. Order is binding, because each phase's starting state is the
previous one's end state.

Extend `ShellBehaviourPhase`, `next()`, `expected_milestone`,
`EXPECTED_MILESTONES`, `TERMINAL_STAGE` and the driver's `phases` list
**together**.

`webview_smoke/shell_behaviour.rs` is at 366 ELOC, and eleven phases will push it
past ~450. **Move the phase table into a submodule** rather than squeezing under
the gate, as slice 2's handoff §7 anticipated.

## 5. Absence needs an observation window · **[Binding]**

C1, D1, E2, E4 and F2 each assert that something **does not** happen. Slice 2
established the rule: an absence cannot be proven by waiting for a positive
condition. A focus claim or restore can land on a later frame than the event the
check waited for.

Use the driver's existing `ABSENCE_OBSERVATION_FRAMES` constant for every one of
them. Do not add a second constant or an inline count.

## 6. What must never be activated · **[Binding]**

### 6.1 Native dialogs

"Open Folder" (app menu), "Export HTML" (tools menu) and the header's "Save As"
open `rfd` portal dialogs. A portal dialog **escapes `xvfb` onto the owner's live
desktop** (RFC-042 §10). This slice clicks inside the app menu for the first time
(E3), which is why it needs a stable handle rather than a position or a
translated label.

**Permitted production change, and the only one:** two static identifying `id`
attributes, with constants in `shell_focus.rs`:

- the app menu's Settings item: `id="app-menu-settings"`;
- the Settings screen's Close button: `id="settings-close"`.

No handler, class, order or behaviour changes. A source-text test must pin both
attributes to the right element. For example, the Settings `id` must appear inside
the button whose handler submits `SourceCommand::OpenSettings`.

**The click guard.** Before any `.click()` inside the app menu:

- the selector must match **exactly one** element;
- that element must be inside `#app-overflow-menu`;
- it must carry the Settings `id`.

If any check fails, fail naming what was found, and click nothing.

### 6.2 State-destroying controls

- **The conflict banner's three actions.** Keep my version overwrites the file on
  disk. Reload discards the unsaved edit. Never click them, and **dispatch no Enter
  or Space anywhere in F**.
- **The app-bar logo** (`#app-bar-logo-trigger`). E2 asserts focus on it, but its
  `onclick` submits `CloseWorkspace`. Never click it.
- **Recovery's Restore and Discard.** Only Skip all (§4.1).
- **Settings' Save.** It writes settings and resets the mode. Only Close.

## 7. Non-change scope · **[Binding]**

- Production code, apart from §6.1's two attributes. In particular:
  `mode_tabs.rs`'s handlers, `app.rs`'s close handlers, `settings_screen.rs`'s
  and `recovery_screen.rs`'s focus effects, and `conflict_banner.rs`.
- **If a contract fails because the app is wrong, stop and report.** Do not fix
  it here. Task 014 and task 017 are the precedents. §2's correction describes
  where I expect a real failure is most likely, if one exists.
- `driver.js`, `--webview-smoke`, the RFC-041 step, and the pin-registry block.
  `webview-smoke-driver-parity.test.mjs` must pass unmodified.
- The search input's entry focus (task 017 review §6). It is not in §8 C–F, and
  adding it here would widen the slice without an owner decision. It stays on my
  list.
- Version numbers, `CHANGELOG.md`, `ROADMAP.md`,
  `docs/src/manual-release-checklist.md`.

## 8. Determinism · **[Binding]**

Poll a named condition with a timeout; never sleep. Every timeout message reports
the state **at timeout**, and the state at the triggering action where that helps
(slice 2's `waitFor` description functions).

Include `document.activeElement` in every message. Also include:

- for C: every tab's `aria-selected`;
- for E: which region is present;
- for F: the banner's presence and button count, and `.dirty-dot`.

## 9. Required tests · **[Binding]**

1. Driver unit tests for every new phase, using the existing shared fakes under
   `crates/bekoedit-app/js/test/`. Extend them; do not fork them. Each contract
   needs a pass case and at least one failing case that names its contract.
2. **Headless proofs for each absence**, forced in the fakes, as slice 2 did for
   contract 7:
   - a tab that activates one frame after an arrow key (C1);
   - a restore to the trigger one frame after the editor takes focus (D1);
   - a focus claim that takes focus from the trigger one frame after Settings
     closes (E4), and from the logo one frame after Recovery exits (E2);
   - a banner that focuses its first action one frame after it appears (F2).

   Each must fail with its contract's message. Prove each test by removing the
   observation window: the test must then pass wrongly.
3. Rust tests for the phase machine, mirroring `shell_behaviour/tests.rs`,
   including the Rust-side write's placement between F1 and F2.
4. **App-code mutations in CI**, each on its own throwaway branch, each failing the
   contract it targets and naming it:

   | Mutation | Must fail |
   |---|---|
   | The tablist's arrow handler also submits `SwitchMode` for the focused tab (automatic activation) | C1 |
   | The app frame's `onfocusin` skips `release_menu_focus` **when the focus target is inside the source editor**, but still sets `open_menu` to `None` | D2 (and not D1) |
   | `ConflictBanner` focuses its first action when mounted (RFC-042 §7.6's original, withdrawn text) | F2 |
   | `recovery_screen.rs`'s `use_drop` does not restore to the logo | E2 |

   The second mutation matters most. It is exactly the failure D1 cannot see, and
   the only reason D2 exists.

   **Why that mutation is conditional on the target.** An unconditional "no
   release" would also break slice 2's contract 7, which closes both menus through
   the same `onfocusin` onto a tree row. Authority would then still be held when
   C2's click claims editor focus, so the run would fail at C2 and credit the
   failure to the wrong phase. Slice 2's Escape mutation had the same problem.
   Scoping the mutation to editor targets keeps every earlier close releasing, so
   the first refused claim is D2's.

   E4 has no mutation here, because the app code under it may already be wrong
   (§2). If E4 passes on the first green run, add a fifth mutation: `close_settings`
   does not call `focus_element`. If E4 fails, stop and report instead.

## 10. Landing it · **[Binding]**

- Extend the existing second run. No third run mode.
- The step stays `continue-on-error`. **Its promotion clock restarts at this
  merge**, because the phase set changed (RFC-044 §10). The restart goes in a
  comment-only commit on approval, as in slices 1 and 2.
- The clock at handoff time: run 1 is `35111768240` (`59f21b7`), and run 2,
  `35113268467` (`472b13f`), is not yet written into `ci.yml`. Record the count
  reached, read from the outcome lines of the `main` push runs since, under
  "previous clocks".

## 11. Acceptance criteria · **[Binding]**

1. C1–C2, D1–D2, E1–E4 and F1–F2 are all in the second run, green in CI.
2. C2 activates through `.click()`, not a synthetic Enter (§3.3).
3. Every absence is observed over `ABSENCE_OBSERVATION_FRAMES` (§5), with its
   headless proof (§9.2).
4. Recovery phases run first, seeded through `RecoveryStore::save`. The conflict
   write is Rust-side, between F1 and F2. Autosave is seeded to one day.
5. §6's guards are present. No dialog-opening control, banner action, logo,
   Restore, Discard or Save is ever activated.
6. The only production changes are §6.1's two attributes, and a source test pins
   them.
7. §9.4's mutations each fail their named contract.
8. The parity test is unmodified and green. The RFC-041 step is unchanged and
   blocking.

## 12. Stages, checkpoint, CI and merge

**Stage 1: C and D.** These use existing harness machinery only.

- Push, reach green or a stop-and-report, and run the C1 and D2 mutations.
- Then write an **interim** review request:
  `.git-exclude/review-request/<date>-rfc-044-slice-3-stage-1-tabs-and-authority.md`.
- Do not start stage 2 until I have replied.

**Stage 2: E and F.** This adds the §4 harness additions and §6.1's attributes.

Throughout:

- Branch, commit, push and draft PR are pre-authorized.
- If `main` moves in a file this slice touches, merge it in; never rebase.
- Merging needs an explicit instruction under the merge procedure.

## 13. Review-request format

Final:
`.git-exclude/review-request/<date>-rfc-044-slice-3-tabs-authority-screens-conflict.md`.

**Lead with:**

- **F2's result**, since it is the one assertion in RFC-044 that protects user
  data;
- **E4's result**, since it is the place §2 predicts a real defect if one exists;
- **the D2 mutation's named failure**, since it is what separates D from a
  duplicate of slice 2's contract 7.
