# RFC-048 handoff — slice 5: Tidy, keyboard, accessibility, and surfacing

**Governing RFC:** [RFC-048](../../accepted/RFC-048-form-mode-table-editing.md) §5.2 (Tidy), §5.3, §5.4, §6, §9
**Slice:** 5 of 5, the last. Slices 1 to 4 are done.
**Baseline:** `main` at `7a3a29d` or later (task 059 merged; run `37465769065` green).
**Status:** inherited from RFC-048 (Accepted 2026-10-02)
**Date:** 2026-10-06
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh`.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Finish the grid:

- an **explicit** Tidy;
- full keyboard use;
- the accessibility points carried from slices 2 to 4;
- documentation that matches what the app now does.

After this slice, RFC-048 moves to `done/`.

## 2. Required · **[Binding]**

### 2.1 Tidy table: the one operation that rewrites the table, and only on request

- **A "Tidy table" action** in the table's own UI, next to Add row.
  **Never automatic** (RFC-048 §10 Q2).
- **It re-pads every cell** so the pipes line up, by **display width**: East
  Asian wide characters count as 2. Use `unicode-width`, which is already in
  `Cargo.lock` at 0.2.2 through another crate. A direct dependency on it is
  allowed. **Report the `Cargo.lock` delta,** which should be none, or the
  dependency line only.
- **Formatting rules:**
  - one space on each side of every cell;
  - delimiter cells as long as their column, with alignment colons kept;
  - the leading/trailing-pipe style kept;
  - line endings kept;
  - **escaped `\|` and every cell's own text unchanged.**
- **Long rows keep their extra cells;** short rows are not padded with new
  cells. Tidy changes whitespace and dashes only.
- **Re-parsed and verified like every other operation:** the same cells, the
  same text and the same alignment. Refuse otherwise.
- **Tidy on an already tidy table changes nothing:** it is a no-op, not a
  rewrite with the same bytes.

### 2.2 The keyboard

**[Advisory]** in the details, **[Binding]** that each point works:

- **Tab** moves to the next cell and **Shift+Tab** to the previous one, across
  rows.
- **Tab in the last cell** moves focus to "Add row". It does not add a row by
  itself.
- **Enter** in a cell commits it and moves down. On the last row, it stays.
- **Escape** in a cell leaves the grid's cell focus, to the table's first
  actions button.
- **The row and column menus,** reachable as in slices 3 and 4.
- **Typed text is committed before focus moves,** with no loss. The task 051
  flush already covers shortcuts. Tab is a blur, so `change` fires; prove it in
  the scenario.

### 2.3 The accessibility points carried forward

- **Data-cell labels use the header's plain text,** as slice 4 did for the
  column menu. So `**Name**, row 2` becomes "Name, row 2" (slice 2's note).
- **An empty header** gives "Column 3, row 2", not ", row 2".
- **An open row or column menu closes on window `resize`** (task 058's note).
- **No visible first-frame flicker** before a menu is placed. For example,
  `visibility: hidden` until placed (task 058's note).

### 2.4 Documentation

**`docs/src/editing-modes.md` is out of date** for Form Mode:

- its block table has no table row, and its "Raw Markdown Islands" list still
  names **tables**;
- "Edits commit when a field loses focus (or on Enter)" no longer covers
  shortcuts, or the toolbar.

Fix both, and add a short **Tables** section:

- cells are edited as Markdown text;
- insert, delete and move rows and columns;
- alignment;
- Tidy;
- the keyboard;
- which tables stay raw, and why (inside a list or quote, or one the parser and
  the grid read differently).

**README:** one line on table editing, in the style of its neighbours.

### 2.5 One end-to-end scenario, blocking

Add `table_keyboard_and_tidy`. Every click passes task 058's guard, and every
save is a real Ctrl+S:

1. Tab through a row with real keystrokes. Type into each cell, Shift+Tab back,
   and save. **Assert exact bytes, and that no typed text was lost.**
2. Tidy a deliberately ragged table **with Japanese text**. Save, and assert
   the exact aligned bytes.
3. Tidy again, and assert that **no save is needed**: the document did not
   change.

## 3. Prohibited · **[Binding]**

- Tidy running without an explicit request.
- Tidy changing any cell's text, escaping, alignment or line endings.
- Weakening any assertion, or making a step non-blocking.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 4. Evidence and review request · **[Binding]**

- **Tidy byte-level tests over the corpus,** including:
  - Japanese and mixed-width text;
  - `\|`;
  - formatted cells;
  - short and long rows;
  - CRLF;
  - no final newline;
  - no outer pipes;
  - each alignment.

  **Plus the idempotence test.**
- **Mutations, one each:**
  - Tidy uses byte length instead of display width;
  - Tidy drops alignment colons;
  - Tidy pads a short row with new cells;
  - Tab no longer commits before moving, which the scenario must catch.
- §2.3's label tests.
- The docs diff.
- **What you expect at the merge push.**
- A CHANGELOG `### Added` entry, for example: "Tidy a Form Mode table's
  columns on request, with Japanese text aligned by display width; move
  between cells with Tab, Shift+Tab and Enter."
- **Every local gate,** including `scripts/check-eloc.sh`, and
  `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-048-slice-5-tidy-keyboard-and-surfacing.md`.
Give the full tip hash and its base.
