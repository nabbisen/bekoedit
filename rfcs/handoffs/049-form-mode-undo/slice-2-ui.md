# RFC-049 handoff — slice 2: the UI

**Governing RFC:** [RFC-049](../../accepted/RFC-049-form-mode-undo.md) §5.3–§5.5, §6, §9
**Slice:** 2 of 2, the last. Slice 1, the core, is on `main` (`ce8d4bf`).
**Baseline:** `main` at the 0.18.1 release (`27292bc`) or later.
**Status:** inherited from RFC-049 (Accepted 2026-10-07)
**Date:** 2026-10-19
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh` and the task 063 check.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Make slice 1's Undo and Redo reachable in Form Mode: buttons, shortcuts,
notices, the mode-switch rule, a blocking end-to-end scenario, and the docs.
After this slice, RFC-049 moves to `done/`.

## 2. Required · **[Binding]**

### 2.1 Buttons

- **Undo and Redo buttons in the Form Mode header,** disabled when
  `can_undo_form()` / `can_redo_form()` is false. Labels in both languages
  through `i18n.rs`, with accessible names.
- **Placement and icons: [Advisory].** Next to the existing Form controls, in
  their style.

### 2.2 Shortcuts

- **Ctrl+Z → Undo, Ctrl+Shift+Z → Redo, and Ctrl+Y → Redo too** (Cmd on macOS
  for the first two), **only while Form Mode is shown and the active element
  is not a text field** (`input`, `textarea`, or `contenteditable`).
- **Inside a field, the browser's own undo of uncommitted typing is kept**
  (§9 Q1). The decision is made in `assets/shortcuts.js`, next to the
  existing relay.
- **Text Mode is untouched.** CodeMirror keeps its own Ctrl+Z.
- **An open IME composition** refuses the shortcut, as the other shortcuts do
  (`DiscardReason::Composing`).

**Note what task 065 recorded:** after Ctrl+2 into Form Mode, nothing has
focus, so the active element is `body`. That is "not a text field": Ctrl+Z
there **is** Form undo. Make sure it works in that state.

### 2.3 Notices

- **A refused Undo or Redo** (`SessionError::FormHistoryStale`) raises one
  notice in both languages, through task 056's refusal path. For example:
  "Nothing was undone: the document changed since that edit." Never silent.
- **An empty stack** is not an error: the button is disabled, and the shortcut
  does nothing.
- No document text in any notice, log or trace.

### 2.4 Lifetime

- **Leaving Form Mode clears the history** (`clear_form_history()`, §9 Q3), by
  any route: the tabs, Ctrl+1/3/4, and Split.
- Opening another document or reloading already clears it, by construction.
  Do not add a second mechanism.

### 2.5 End-to-end scenario, blocking

Add `form_undo_redo` to the release checks. Every click passes task 058's
guard, every key is a real keystroke, every save a real Ctrl+S:

1. In Form Mode, edit a table cell, then delete a table row.
2. Move focus out of every field (a real click on a non-field area of the
   Form view), then **Ctrl+Z twice**. Save. **Assert the original bytes.**
3. **Ctrl+Shift+Z twice.** Save. Assert the bytes after both edits.
4. Click into a paragraph field, type, and press **Ctrl+Z inside the field**:
   only the typing is undone, and the committed history is unchanged. Assert
   that the Undo button's state did not change.
5. **The buttons:** one Undo by a real click on the button. Save, assert.

### 2.6 Documentation

`docs/src/editing-modes.md`: a short **Undo** paragraph in the Form Mode
section. Which edits it covers; Ctrl+Z inside a field versus outside one; the
history clears when you leave Form Mode; a refused undo, and why.

## 3. Prohibited · **[Binding]**

- Any change to Text Mode, CodeMirror, or the source-sync controller.
- Any change to slice 1's guard, or a second history.
- Weakening any assertion, or making a step non-blocking.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 4. Evidence and review request · **[Binding]**

- **JS unit tests** for the `shortcuts.js` decision: Form Mode with focus on
  `body`, on a button, in an `input`, in a `textarea`, in a `contenteditable`;
  Text Mode; an open composition.
- **Rust tests:** the shortcut and the buttons reach `undo_form` /
  `redo_form`; the refusal raises the notice; leaving Form Mode by each route
  clears the history.
- **Mutations, one each:** the field check removed from `shortcuts.js` (a JS
  test and step 4 must catch it); the mode-switch clear removed; the refusal
  notice removed.
- **What you expect at the merge push:** nineteen scenarios, including
  `form_undo_redo`.
- **A CHANGELOG `### Added` entry,** for example: "Undo and Redo in Form Mode:
  Ctrl+Z and Ctrl+Shift+Z (or the buttons) step back and forward through
  committed edits. Inside a field, Ctrl+Z still undoes your typing there."
- **Every local gate,** Rust tests with `CI` unset, and
  `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-049-slice-2-ui.md`.
Give the full tip hash and its base.
