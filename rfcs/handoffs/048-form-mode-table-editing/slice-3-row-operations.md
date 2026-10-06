# RFC-048 handoff — slice 3: row operations

**Governing RFC:** [RFC-048](../../done/RFC-048-form-mode-table-editing.md) §5.2, §5.3, §5.4, §6
**Slice:** 3 of 5. Slices 1 and 2 are done: task 045, and `c1aba2c` plus
`8ddd1d4`.
**Baseline:** `main` at `d723da2` or later, which is green (run `37110705407`).
**Status:** inherited from RFC-048 (Accepted 2026-10-02)
**Date:** 2026-10-03
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.**

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Rows can be inserted above or below, deleted, and moved up or down in the
Form Mode grid. **Each operation is one source patch that changes the minimum
bytes** (RFC-048 §5.2).

## 2. Required · **[Binding]**

### 2.1 Four edits, each resolved against `gfm::table_cell_ranges`

New `FormBlockEdit` variants. Names **[Advisory]**:

- `InsertTableRow { at, position: Above | Below }`;
- `DeleteTableRow { row }`;
- `MoveTableRow { row, direction: Up | Down }`.

| Operation | Bytes changed |
|---|---|
| Insert above/below | **one new line**, using the document's line ending. It has as many empty cells as the header, in the table's leading/trailing-pipe style. **Every existing line is byte-identical.** |
| Delete | **that row's line and its line ending, only.** At the table's last line with no trailing newline, remove the preceding line ending instead, so no line is left dangling. |
| Move up/down | **the two lines swap.** Each line's own bytes are kept exactly, and the line endings stay where they were. |

- **The header row (row 0)** cannot be deleted, moved, or have a row inserted
  above it. Refuse with `UnsupportedEditOperation`.
- **A data row cannot move above the header.**
- **Deleting the only data row is allowed.** A table with a header and no rows
  is valid GFM.
- **Every result is re-parsed.** It must parse as a table, with the expected
  number of rows, and every **untouched** cell's text unchanged. If not,
  **refuse; do not apply.**
- **`AddTableRow` stays as it is.** It is Insert Below on the last row; keep
  it, or route it through the new code. Either way its tests must keep passing.

**The short-row case from task 045's re-review §2** (`| 1 |Z`) is yours to
decide in this slice. Insert writes full rows, so new rows never have it. Say
whether you change the in-cell edit for missing cells, and why.

### 2.2 The grid UI

- **Each data row gets an actions button** (row number, or a small menu
  glyph) that opens a menu with:
  - Insert row above;
  - Insert row below;
  - Delete row;
  - Move up;
  - Move down.
- **Unavailable items are disabled:** Move up on the first data row, Move down
  on the last.
- **Use the app's existing menu pattern** (`role="menu"` / `"menuitem"`, as
  `app_bar.rs`). Keyboard:
  - Enter or Space opens the menu;
  - the arrow keys move between items;
  - Escape closes it and returns focus to the button.
- **Labels in both languages,** in `i18n.rs`. The new-key test from task 044
  catches any missing one. Each button names its row, for example "Actions for
  row 2" / 「行 2 の操作」.
- **After an insert,** focus goes to the new row's first cell. **After a
  delete,** it goes to the same column in the row that took its place, or in
  the previous row. **After a move,** it follows the moved row.
  - The stable block key from task 052 keeps the grid mounted. Mechanism
    **[Advisory]**.

### 2.3 Undo

**RFC-048 §5.3 says this slice checks how a row operation appears in undo,
and does not assume it.**

- Find out what Form Mode's undo does today, if anything, for a cell edit.
- Then do the same for a row operation. Report it.
- **If a row operation behaves differently from a cell edit,** make it
  consistent, or report it before building more.

### 2.4 One end-to-end scenario, blocking

Add `table_row_insert_and_delete` to the release checks:

1. Open a seeded table in Form Mode.
2. **Insert a row below the first data row, through the real UI:** a real
   click on the row's actions button, then the menu item.
3. Type into the new row's first cell with real keystrokes. **Save with
   Ctrl+S.** Autosave is off in this run mode, from task 054.
4. Assert the saved bytes exactly: one new line, and every other byte
   unchanged.
5. Delete that row through the menu, save again, and assert the **original
   bytes** are back.

## 3. Prohibited · **[Binding]**

- Column operations, alignment, and Tidy: slices 4 and 5.
- **Any change to bytes outside the stated range,** including re-padding.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 4. Evidence and review request · **[Binding]**

- **Byte-level tests for each operation,** asserting the exact output, over a
  corpus:
  - with and without outer pipes;
  - padded and unpadded;
  - CRLF and LF;
  - a table that ends the file with no final newline;
  - a table followed by a paragraph;
  - Japanese text;
  - `\|` in a cell;
  - formatted cells.
- **The refusal cases:** the header row, moving out of range, and a re-parse
  mismatch, if you can construct one.
- **Mutations, one each:**
  - insert re-pads an existing line;
  - delete leaves a blank line;
  - move swaps line endings;
  - the header guard is removed.
- §2.3's undo finding.
- The scenario, and **what you expect at the merge push**.
- A CHANGELOG `### Added` entry, for example: "Insert, delete and move table
  rows from the grid in Form Mode."
- The local gates, and `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-048-slice-3-row-operations.md`.
Give the full tip hash and its base.
