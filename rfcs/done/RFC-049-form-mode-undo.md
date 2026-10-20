# RFC-049: Form Mode undo

**Project:** bekoedit
**Status:** Implemented — on `main`, not yet released. Approved by the owner on
2026-10-07, as written, so each §9 question takes its proposed answer. Drafted
2026-10-06, after the owner chose the contained option ("B") over a single
history shared with Text Mode. Merged in two slices, the last green on `main`
at `1ef35d6` (run `39513465302`, 2026-10-21):

- **Slice 1** (`ce8d4bf`): the core: the records, the guard, and the
  restamping rule now stated in §5.2;
- **Slice 2** (`6f01126`, then `1ef35d6` for the scenario's step 4): the
  buttons, the shortcuts, the notice, the mode-switch clear, and the docs.

**One open fact, about §9 Q1's premise.** Ctrl+Z inside a field is left to the
field, and never changes the Form history: that is asserted end to end. But in
CI (WebKitGTK under Xvfb, `xdotool` input), the field's own undo reverted
nothing in eight presses. Whether a user at a real keyboard gets the browser's
in-field undo is not yet known. The owner is asked to try it.
**Handoffs:** [`handoffs/049-form-mode-undo/`](../handoffs/049-form-mode-undo/)
**Track:** Editing
**Priority:** Medium — today a committed Form Mode edit cannot be stepped back
**Date:** 2026-10-06
**Related RFCs:** [RFC-015](../done/RFC-015-sourcepatch-engine-and-source-preserving-mutation.md), [RFC-016](../done/RFC-016-form-mode-mvp-surface-and-safe-editable-blocks.md), [RFC-047](../done/RFC-047-user-commands-during-editor-transitions.md), [RFC-048](../done/RFC-048-form-mode-table-editing.md)

---

## 1. Summary

**Undo and Redo for edits committed in Form Mode.** Each is one inverse source
patch, applied only when the document is exactly as the edit left it. Text
Mode, its editor and the source-sync controller are not touched.

## 2. Motivation

**Form Mode has no undo for a committed edit.** That covers cell and row
operations, deletes, toggles, and text commits. RFC-048 slice 3 found this and
reported it. Only the browser's own per-field undo exists, for keystrokes not
yet committed.

**Text Mode has undo,** through CodeMirror's own history, inside the WebView.

**The safety nets today** are coarse:

- document history, the last 50 saves, restorable;
- crash-recovery snapshots.

## 3. Goals

1. **Undo and Redo of committed Form Mode edits,** each as one source patch.
2. **Never damage a document.** An undo that no longer fits is refused, with a
   notice, and is never applied.
3. **No change to Text Mode,** its editor, or the source-sync controller.

## 4. Non-goals

- **One history shared by Text and Form Mode.** It was considered and
  declined: it would reroute every edit through a shared history, inside the
  controller that RFC-041 and RFC-047 stabilised.
- Undo across a document switch, an external reload, or a restart.
- Undo of saves, file operations, or workspace actions.

## 5. Design

### 5.1 The record of one edit

**What exists today:**

- `Session::apply_form_edit` resolves a `FormEditCommand` to one `SourcePatch`,
  and applies it with `apply_patch`, which returns the range the replacement
  now covers.
- `after_mutation` increments `revision`.

**For each Form edit that applies, record:**

- `revision_after`: the session revision once the edit is applied;
- `range_after`: the byte range the replacement now covers;
- `replaced`: the exact bytes the edit removed.

### 5.2 Undo, Redo, and the one guard

**Undo** takes the newest record. Then:

- **only if `session.revision == revision_after`,** it applies a `SourcePatch`
  that replaces `range_after` with `replaced`, through the same `apply_patch`;
- otherwise it is refused, with a notice, and the stack is cleared.

**That revision check is the whole safety argument.** Any other mutation
increments `revision`, so a stale record can never apply:

- a Text Mode edit;
- a restored snapshot;
- an external reload;
- another document.

**Redo** is symmetric. An undo pushes its own inverse onto the redo stack. Any
new Form edit clears Redo.

**Which record is current** *(amended 2026-10-19, after slice 1)*. Read
literally, the check above refuses every second Undo: the first Undo is itself
a mutation, so the record beneath no longer matches the revision. Slice 1
found this and built the rule below. A record's `revision_after` is the
revision at which it is **current**:

- **After a successful Undo or Redo,** the record now on top of that stack is
  restamped to the new revision. The document is then exactly as that record
  left it, so the restamp only states a fact.
- **A new edit clears the Undo stack first if its top is not current.** So the
  record beneath any record was current immediately before that one applied,
  and a chain never spans a foreign mutation.
- **A Form edit that changes no byte** still advances the revision. Each top
  that was current is restamped, since the bytes are unchanged.
- **Every operation updates both tops together,** so whenever both stacks hold
  records, their tops are both current or both stale.

The revision check is still the only way a record applies. A top that is not
current is refused, and both stacks are cleared.

### 5.3 Lifetime

- **The stacks belong to the open document,** and are cleared when:
  - another document opens;
  - the mode switches away from Form;
  - the file is reloaded.
- **They survive a save.** Undoing past a save makes the document dirty again,
  as any edit does.
- **They hold at most 100 records** (§9 Q2). The oldest is dropped. Memory is
  only the replaced bytes.

**Where the stacks live** *(clarified on acceptance, 2026-10-07)*. They are
fields of `DocumentSession` itself. Opening a document, a conflict's "reload
from disk", and a restore that opens a file all **replace** the session with a
new one, whose `revision` starts again at 1. So §5.2's argument, that any other
mutation increments `revision`, holds only within one session object. Stacks
stored outside it, in the store or the app, could meet a reloaded document
whose revision has climbed back to a recorded `revision_after`, and apply a
stale patch. Inside the session, a replaced session takes its stacks with it,
by construction.

### 5.4 Controls

- **Undo and Redo buttons** in the Form Mode header, disabled when empty, and
  labelled in both languages.
- **Ctrl+Z and Ctrl+Shift+Z (Ctrl+Y on Windows), Cmd on macOS,** route to Form
  undo **only when focus is not in a text field**.

  Inside a field, the browser's own undo of uncommitted typing is kept (§9 Q1).
  The decision is made in `shortcuts.js`, which sends the shortcut only when
  `.form-mode` is shown and the active element is not an `input` or
  `textarea`.

### 5.5 Refusals

An undo that cannot apply raises one notice, in both languages, for example:
"Nothing was undone: the document changed since that edit." This follows task
056's refusal path. It is never silent.

## 6. Testing

- **Core, headless:**
  - each Form edit kind, undone and redone, restores the exact bytes: text,
    toggles, cell, row and column operations, delete, and Tidy;
  - a Text Mode edit in between makes undo refuse;
  - the stacks clear on a document switch and a mode switch;
  - the 100-record bound.
- **Property:** a random sequence of Form edits, then the same number of undos,
  returns the original bytes exactly.
- **WebView end-to-end, blocking:**
  - edit a cell and delete a row, then Undo twice by a real Ctrl+Z with focus
    outside any field, save with Ctrl+S, and the original bytes are back;
  - Ctrl+Z inside a field undoes only the typing there.

## 7. Risks

- **Ctrl+Z ambiguity.** Mitigated by the focus rule, and by the visible
  buttons.
- **A future mutation path that forgets to bump `revision`** would break the
  guard. Mitigated by a test that every `Session` mutation increments it.

## 8. Slices

1. **Core:** the records, Undo and Redo, the guard, the lifetime, and the
   tests.
2. **UI:** the buttons, the shortcuts, the notices, the end-to-end scenario,
   and the docs.

## 9. Questions for the owner

*Answered 2026-10-07: the owner accepted the RFC as written, so every proposed
answer below stands.*

1. **Ctrl+Z inside a Form field:** keep the browser's own undo of uncommitted
   typing there, with Form undo only outside a field? *Proposed:* yes.
2. **History depth:** 100 records? *Proposed:* yes.
3. **Clear the history when you leave Form Mode?** *Proposed:* yes. It is the
   simplest safe rule, and the revision guard would refuse those undos anyway.
4. **Timing:** after 0.18.0? *Proposed:* yes, and that is the owner's earlier
   direction.
