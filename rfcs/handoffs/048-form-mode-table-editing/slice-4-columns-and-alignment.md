# RFC-048 handoff — slice 4: columns and alignment

**Governing RFC:** [RFC-048](../../done/RFC-048-form-mode-table-editing.md) §5.2, §5.3, §5.4, §6
**Slice:** 4 of 5. Slices 1 to 3 are done (tasks 045, 056 and 058, and the
slice 2 and slice 3 commits).
**Baseline:** `main` at `8fffd85` or later, which is green.
**Status:** inherited from RFC-048 (Accepted 2026-10-02)
**Date:** 2026-10-06
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.**

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Columns can be inserted left or right, deleted, and moved left or right, and a
column's alignment can be set, from the Form Mode grid. As with rows, **each
operation is one source patch that changes the minimum bytes**, and the
re-parse safety net from slice 3 refuses anything unexpected.

## 2. Required · **[Binding]**

### 2.1 Four edits, resolved against `gfm::table_cell_ranges`

| Operation | Bytes changed |
|---|---|
| Insert left/right of column *c* | **one new cell in every line,** including the delimiter line, which gets `---`. A new cell is written as ` ` + ` \|` (one space each side, as slice 3's review settled for empty cells), in each line's own pipe style. **No other cell's bytes change.** |
| Delete column *c* | **that cell, and exactly one adjacent pipe,** in every line, including the delimiter. The last remaining column cannot be deleted. |
| Move column *c* left/right | **the two cells swap in every line,** including the delimiter, so **alignment moves with its column**. Each cell's own bytes, including its padding, travel unchanged. |
| Set alignment of *c* | **that column's delimiter cell only**: `:---` (left), `:---:` (centre), `---:` (right), `---` (none). **Keep its dash count** where the form allows it, with a minimum of three dashes. |

- **Rows shorter than the header:**
  - insert and delete act only on cells the row has. A row that lacks column
    *c* is left alone;
  - move swaps only when the row has both cells;
  - otherwise, that line is left alone.

  **Pin each case with a test.**
- **Rows longer than the header** keep their extra cells untouched.
- **Every result is re-parsed** (slice 3's `verify_row_operation`, or its
  column twin). The column count is the expected one, and every untouched
  cell's text is unchanged. **Refuse otherwise; never apply.**
- **An escaped `\|`** inside a cell is never a boundary, which
  `table_cell_ranges` already guarantees. Include it in the corpus.

### 2.2 The grid UI

- **Each column header gets an actions button,** with the same menu pattern
  as slice 3's rows:
  - Insert column left;
  - Insert column right;
  - Delete column;
  - Move left;
  - Move right;
  - Alignment: Left, Centre, Right, None, with the current one marked.

  Unavailable items are disabled: Move left on the first column, Move right on
  the last, and Delete on the only column.
- **The menu is `position: fixed`,** placed like slice 3's row menu, which
  task 058 fixed. **Reuse that placement code**; do not copy it.
- **Keyboard and labels** as in slice 3, in both languages. Each button names
  its column by header text, for example "Actions for column Name".
  - **Label by the header's plain text:** strip Markdown markers, so that
    `**Name**` reads "Name". That closes slice 2's accessibility note, for
    columns at least.
- **The grid shows each column's alignment,** through the cell inputs'
  `text-align`.
- **Focus after each operation:**
  - an insert puts focus in the new column's header cell;
  - a delete puts it on the actions button of the column that took its place,
    or of the previous one;
  - a move follows the column.

### 2.3 One end-to-end scenario, blocking

Add `table_column_operations`. Every click goes through the real UI and passes
task 058's click guard; every save is a real Ctrl+S, with autosave off in this
run mode:

1. Insert a column to the right of the first, type a header into it, and save.
   Assert the exact bytes.
2. Set its alignment to Centre, and save. **Assert only the delimiter cell
   changed.**
3. Move it left, and save. Assert the swapped bytes.
4. Delete it, and save. **Assert the original bytes are back.**

## 3. Prohibited · **[Binding]**

- **Tidy** (re-padding the whole table). That is slice 5, and is only ever
  explicit.
- **Any change to bytes outside the stated range,** including re-padding
  neighbouring cells to line up.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 4. Evidence and review request · **[Binding]**

- **Byte-level tests for each operation, asserting exact output,** over the
  slice 3 corpus:
  - with and without outer pipes;
  - padded and unpadded;
  - CRLF and LF;
  - no final newline;
  - followed by a paragraph;
  - Japanese text;
  - `\|`;
  - formatted cells;
  - short and long rows.
- **The refusal cases:** deleting the only column; moving out of range.
- **Mutations, one each:**
  - insert re-pads a neighbouring cell;
  - delete removes both adjacent pipes;
  - move leaves the delimiter in place;
  - alignment rewrites the whole delimiter line.
- The scenario, and **what you expect at the merge push**.
- A CHANGELOG `### Added` entry, for example: "Insert, delete and move table
  columns, and set a column's alignment, from the grid in Form Mode."
- The local gates, and `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-048-slice-4-columns-and-alignment.md`.
Give the full tip hash and its base.
