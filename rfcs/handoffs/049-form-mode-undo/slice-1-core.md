# RFC-049 handoff — slice 1: the core

**Governing RFC:** [RFC-049](../../accepted/RFC-049-form-mode-undo.md) §5.1–§5.3, §6, §7
**Slice:** 1 of 2. Slice 2 is the UI: buttons, shortcuts, notices, the
end-to-end scenario and the docs.
**Baseline:** `main` at the 0.18.0 release commit or later. **Do not start
before the 0.18.0 tag** (RFC-049 §9 Q4).
**Status:** inherited from RFC-049 (Accepted 2026-10-07)
**Date:** 2026-10-07
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh`.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Undo and Redo of committed Form Mode edits, in `bekoedit-core`, headless and
fully tested. **No UI in this slice:** nothing in `bekoedit-app` calls it yet,
apart from what §2.5 allows.

## 2. Required · **[Binding]**

### 2.1 Where the history lives

**The Undo and Redo stacks are fields of `DocumentSession`.** Not the store,
not the app.

**Why this is binding:** `DocumentSession::load` and `from_text` start a new
session at `revision: 1`. Opening a document, a conflict's `ReloadDisk`
(`store.rs`, `*session = DocumentSession::load(..)`) and a history restore all
replace the session. A history kept outside it could meet a reloaded document
whose revision has climbed back to a recorded value, and the guard in §2.3
would then pass a stale patch. Inside the session, replacement discards it by
construction. RFC-049 §5.3 records this.

`DocumentSession` derives `Serialize`, `Deserialize`, `PartialEq` and `Eq`.
**The stacks are not serialized** (`#[serde(skip)]`, empty on deserialize).
Say in the request whether any existing `PartialEq` comparison of sessions
changes meaning.

### 2.2 Recording

In `apply_form_edit`, for each edit that applies, record:

- `revision_after`: `self.revision` once `after_mutation` has run;
- `range_after`: `PatchResult::affected_range`;
- `replaced`: the exact bytes the patch replaced.

**Rules:**

- **Capture `replaced` without risking a panic.** For example, read it with
  `str::get` before `apply_patch`, and discard it if the patch is refused.
  **[Advisory]** in the mechanism.
- **A refused edit records nothing,** and leaves both stacks unchanged.
- **An edit whose replacement equals the bytes it replaces** is not recorded,
  and does not clear Redo.
- **Any other recorded edit clears Redo.**
- **At most 100 records** on each stack; the oldest is dropped.

### 2.3 Undo, Redo, and the one guard

`DocumentSession::undo_form()` and `redo_form()`:

- **Apply only if `self.revision == record.revision_after`.** The patch
  replaces `range_after` with `replaced`, through the same `apply_patch`, with
  `base_revision: self.revision` and `origin: PatchOrigin::FormMode`, then
  `after_mutation`.
- **Otherwise, refuse:** return an error naming the reason, clear **both**
  stacks, and change nothing else.
- **An empty stack** is a distinct, non-error outcome, for example
  `Ok(false)`. Slice 2 disables the button instead of raising a notice.
- **Undo pushes its own inverse onto Redo,** and Redo pushes onto Undo, with
  the new `revision_after`. **Neither one clears Redo.**
- **A Text Mode edit or a restored snapshot** clears nothing itself; the guard
  refuses later. **[Advisory]:** clearing both stacks in `apply_editor_text`
  and `apply_restored_snapshot` is also acceptable, if you prefer it.

**`Store::undo_form(now_ms)` and `redo_form(now_ms)`** pass the same gates as
`edit_form`: refused while a conflict needs a decision, and `after_edit` on
success, so autosave and dirty state behave exactly as for any Form edit.

**`DocumentSession::clear_form_history()`**, for slice 2's mode switch.

### 2.4 Error and trace

- A new `SessionError` variant for the refusal. Its message names the reason,
  and **never contains document text.**
- No toast in this slice; slice 2 adds the notice through task 056's refusal
  path.

### 2.5 Allowed in `bekoedit-app`

Only what the new `Store` methods need to compile. No buttons, no shortcuts.

## 3. Tests · **[Binding]**

**Headless, in `bekoedit-core`:**

- **Each Form edit kind, undone, restores the exact bytes; redone, restores
  the edited bytes:** a text commit, a toggle, a cell edit, row insert, delete
  and move, column insert, delete and move, alignment, block delete, and Tidy.
  Include CRLF, and Japanese text.
- **Undo twice, then Redo twice,** across two different edits.
- **The guard:** a Text Mode edit after a Form edit makes Undo refuse, leaves
  the text untouched, and empties both stacks.
- **A reloaded document** (`ReloadDisk`) has empty stacks, **and** a Form edit
  on it followed by Undo restores the reloaded bytes, not the old ones.
- **Open another document, then back:** empty stacks.
- **A save does not clear the stacks,** and Undo after a save sets `dirty`.
- **A no-op edit** is not recorded, and does not clear Redo.
- **A new edit after an Undo clears Redo.**
- **The 100-record bound:** 101 edits, 100 undos, the 101st is `Ok(false)`, and
  the text is the state after the first edit.
- **Every `DocumentSession` mutator increments `revision`** (RFC-049 §7):
  `apply_text_snapshot`, `apply_editor_text`, `apply_form_edit`,
  `apply_restored_snapshot`, `undo_form` and `redo_form`.

**Property test.** A random sequence of Form edits on a mixed document, then
the same number of Undos, returns the original bytes exactly; then the same
number of Redos returns the edited bytes. **No new dependency:** use a small
seeded generator and print the seed on failure. At least 200 seeds.

## 4. Prohibited · **[Binding]**

- A change to Text Mode, CodeMirror, `shortcuts.js`, or the source-sync
  controller.
- A new dependency.
- Undo applying anything without the revision guard.
- Document text in any error, log or trace.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 5. Evidence and review request · **[Binding]**

- **Mutations, one each, every one caught by a named test:**
  - the guard removed;
  - the stacks moved out of `DocumentSession` into `Store` (the reload test
    must catch it);
  - Undo clears Redo;
  - a refused Undo leaves the stacks;
  - the 100 bound off by one.
- **Every local gate,** with the Rust tests run with `CI` unset:
  `scripts/check-eloc.sh`, fmt, clippy 1.88.0 `-D warnings`, the workspace
  tests, the JS tests, `check-changelog`, `check-rfcs`.
  `git diff --exit-code -- Cargo.lock`, and
  `git diff --stat origin/main...HEAD | tail -1`.
- **No CHANGELOG entry** in this slice: nothing is visible yet. Slice 2 adds
  it.

Review request: `.git-exclude/review-request/<date>-rfc-049-slice-1-core.md`.
Give the full tip hash and its base.
