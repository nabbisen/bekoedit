# RFC-048: Form Mode table editing

**Project:** bekoedit
**Status:** Accepted — approved by the owner on 2026-10-02, the day it was
drafted, after the owner named easy table editing as a key requirement. All of
§10 is answered.
**Handoffs:** [`handoffs/048-form-mode-table-editing/`](../handoffs/048-form-mode-table-editing/)
**Track:** Editing
**Priority:** High — the owner's key requirement, and the current behaviour
both under-delivers and, until task 045, loses data
**Date:** 2026-10-02
**Related RFCs:** [RFC-016](../done/RFC-016-form-mode-mvp-surface-and-safe-editable-blocks.md), [RFC-017](../done/RFC-017-raw-markdown-islands.md), [RFC-027](../done/RFC-027-table-editing-strategy.md)

---

## 1. Summary

Make every well-formed GFM table editable in Form Mode as a grid:

- edit any cell, including cells with **bold**, `code` or links;
- insert, delete and move rows and columns;
- set a column's alignment.

Every operation patches only the bytes it must. Re-padding the whole table
happens only when the user asks for it.

## 2. Motivation

**The owner, 2026-10-02:** "One of the key requirements on this app is to
modify tables easily for users." What Form Mode offers today, measured on
`main` at `6f5f095`:

| Need | Today |
|---|---|
| Edit a cell's text | Yes, **only in a "simple" table** |
| A table with any bold, italic, strikethrough, inline code or inline HTML in **any** cell | **Not editable as a table.** The whole table becomes a raw-text island (`index/blocks.rs`, `classify_table`) |
| Add a row | At the end only |
| Insert a row in the middle; delete or move a row | No |
| Add, delete or move a column | No |
| Set alignment | No |
| Keep the source as written | **No.** Every edit regenerates the table: alignment is erased, text after an escaped `\|` is lost, and a typed `\|` adds a column. **Fixed by task 045**, which is this RFC's slice 1. |

A header in bold (`| **Name** |`) is ordinary in real documents. Any single
such cell makes today's table editing unavailable for the whole table.

RFC-027 deliberately deferred this: "a future decision document chooses
raw-only, simple-grid, or full-grid scope". This RFC is that decision, and it
proposes the full grid.

## 3. Goals

1. Every GFM table that `pulldown-cmark` parses is Form-editable, **except** in
   the cases §5.6 lists.
2. Cell, row, column and alignment operations, each as **one source patch**
   that changes the minimum bytes.
3. Keyboard-complete and screen-reader-labelled.
4. Japanese text in cells is first-class, both in display and in the optional
   tidy.

## 4. Non-goals

- Rich (WYSIWYG) editing *inside* a cell. A cell is edited as its Markdown
  text (§10 Q1).
- Merged cells, multi-line cells, and table extensions beyond GFM.
- Sorting, formulas, or CSV import and export. These may come later as their
  own RFCs.
- Spreadsheet-style multi-cell selection.

## 5. Design

### 5.1 Cells and their boundaries

**Task 045** locates each cell's byte range by splitting on unescaped pipes,
**checked against `pulldown-cmark`**. A table where the two disagree is not
edited: it stays a raw island (§5.6).

- A cell is shown and edited as **its Markdown source text**, for example
  `**Alice**`, with `\|` shown as `|`.
- The Form Mode inline toolbar (bold, italic, code) works in the cell's input,
  as it does in paragraphs.

### 5.2 Patch rules, one patch per operation

| Operation | Bytes changed |
|---|---|
| Edit cell | that cell's content only (task 045) |
| Insert row above/below | one new line |
| Delete row | that line, and its line ending. The header row cannot be deleted. |
| Move row up/down | the two lines swap. Data rows only. |
| Insert column left/right | one new cell in every line, plus `---` in the separator. Other cells unchanged. |
| Delete column | that cell, and one adjacent pipe, in every line. The last column cannot be deleted. |
| Move column left/right | two cells swap in every line |
| Set alignment | that column's separator cell only (`:---`, `:---:`, `---:`, `---`) |
| **Tidy table** (explicit command) | the whole table re-padded by **display width**, so East Asian wide characters count as two. **The only operation that rewrites the table, and only when asked.** |

Every operation:

- keeps the document's line endings;
- keeps the table's leading/trailing-pipe style;
- is validated against a fresh parse after patching. If the result does not
  parse to the expected shape, the patch is refused, not applied.

### 5.3 The grid UI

- Each row and each column has an actions button that opens a small menu with
  the §5.2 operations. Alignment is in the column menu.
- **Keyboard [Advisory, to be fixed in slice 5]:**
  - Tab and Shift+Tab move between cells;
  - Enter moves down;
  - Tab in the last cell offers a new row;
  - the row and column menus are reachable from the keyboard.
- Edits commit on change, as cell edits do today. Each operation is one source
  patch. How it appears in undo follows the existing Form Mode edits; slice 3
  checks this, and does not assume it.

### 5.4 Accessibility

- The table is a real `<table>` with header cells.
- Each input is labelled by its column header and row number, for example
  "Name, row 2".
- Each action button names its target, for example "Actions for column Name".
- Menus follow the app's existing menu pattern (RFC-042).

### 5.5 Display

- The grid shows each column's alignment.
- Cell inputs size to content, with a reasonable maximum.

### 5.6 What stays a raw island

- A table `pulldown-cmark` and §5.1's splitter disagree on.
- A table inside a list or blockquote, where Form Mode does not project its
  container today. It stays as it is.
- Anything malformed, as today. These are honest downgrades, never silent
  rewrites (RFC-017).

## 6. Testing

- **Byte-level tests per operation.** Bytes outside the operation's stated
  range are unchanged, and the result re-parses to the expected shape.
- **A corpus:** tables with and without outer pipes, padded and unpadded, CRLF
  and LF, `\|`, empty cells, formatted cells, and Japanese text.
- **WebView end-to-end:** at least one row operation and one column operation,
  through the real UI on CI, in the release-checks style.
- **Mutation evidence** for each operation's range rule.

## 7. Risks

- **Column operations touch every line.** Bounded by the per-cell ranges, and
  checked by the re-parse.
- **Raw cell text may surprise users** who expect WYSIWYG. Mitigated by the
  toolbar. Revisit if feedback says so.

## 8. Cost

All of it is Rust resolver work plus Form Mode UI. There are no new
dependencies. A display-width function may be needed for Tidy. The
`unicode-width` crate is common, but check whether it is already in the tree
before adding it.

## 9. Slices

1. **Cells as minimal patches**: task 045, a defect fix. It ships in the next
   release; there is no 0.17.1 (§10 Q4).
2. **Formatted cells become editable**: the classification change in §5.1,
   with the toolbar in cells.
3. **Row operations.**
4. **Column operations and alignment.**
5. **Tidy, keyboard, accessibility pass**, docs and the README.

Slices 1–5 target 0.18.0.

## 10. Questions — all four answered 2026-10-02

1. **Cells: Markdown text, or rich text?** **Answered 2026-10-02 by the owner:
   Markdown text (option A)**, with the toolbar.
   *Proposed:* Markdown text, with the
   toolbar. Rich editing inside cells is a much larger editor and risks
   rewriting cell source.
2. **Re-padding: never automatically, with Tidy on request?** **Answered: yes,
   as proposed.** *Proposed:* yes.
   Automatic re-padding rewrites every line on every edit, which fills Git
   diffs. That is the problem this app exists to avoid.
3. **Scope: the full grid (§5.2), or a smaller first step**, such as rows only
   in 0.18.0? **Answered: the full grid, as proposed.** *Proposed:* the full grid across slices 2–5. Each slice is
   independently releasable.
4. **The Microsoft Store:** submit with 0.17.1 (an honest listing, after task
   045), or wait for slices 2–5? *Proposed:* do not wait. **Answered: no 0.17.1
   is cut, to keep release cost down.** Tasks 044 and 045 ship with the next
   release, and the Store submission follows that release.
