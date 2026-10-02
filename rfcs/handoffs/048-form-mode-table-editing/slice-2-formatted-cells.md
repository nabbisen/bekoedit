# RFC-048 handoff — slice 2: formatted cells are editable

**Governing RFC:** [RFC-048](../../accepted/RFC-048-form-mode-table-editing.md) §5.1, §5.6, §6, §10 Q1
**Slice:** 2 of 5. Slice 1 is task 045.
**Baseline:** `main` with **task 045 merged**. Do not start before it is.
**Status:** inherited from RFC-048 (Accepted 2026-10-02)
**Date:** 2026-10-02
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.**

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

**What the owner saw.** A table showed "Table (raw Markdown) — Edited as raw
Markdown to preserve your source exactly". The reason: `classify_table`
(`index/blocks.rs`) demotes the **whole table** to a raw island when any cell
contains emphasis, strong, strikethrough, inline code, or inline HTML.

**After this slice,** such a table is a grid like any other, and each cell is
edited as its Markdown text, for example `**Alice**`. That is the owner's
answer to RFC-048 §10 Q1: option A.

## 2. Required · **[Binding]**

### 2.1 Classification

- A GFM table is `SimpleTable` (form-editable) **unless** task 045's cell
  splitter and `pulldown-cmark` disagree on its cells. In that case it stays a
  `ComplexTable` raw island, as today.
- Inline formatting in cells is **no longer** a reason to demote.
- Tables in containers Form Mode does not project keep their current
  behaviour. Say in the request what that behaviour is.
- **Rename only if it helps** **[Advisory]**. `SimpleTable` now means "a
  form-editable table". A rename across the crate is acceptable, but not
  required.

### 2.2 Projection and editing

- Each cell's input shows **its Markdown source text**, with only `\|`
  unescaped, per task 045.
- Editing it uses task 045's `ReplaceTableCell`, unchanged: the patch is that
  cell's content only. A cell containing `` `a|b` `` or `**x**` round-trips
  byte-exact when the user changes nothing else in it.

### 2.3 The inline toolbar in cells

- Bold, italic and code, and the link button if the toolbar has one, work on
  the **focused cell's** selection.
- **The patch stays inside that cell's content range.** Offsets are relative
  to the cell, not to the table block.
- A typed or inserted `|` is escaped as in task 045.
- Mechanism **[Advisory]**. Two examples:
  - a new edit variant carrying `row` and `col`;
  - computing the new cell text, then reusing `ReplaceTableCell`.
- **One toolbar for the table, acting on the focused cell, is preferred over
  one per cell.**

**One observation, not a requirement of this slice.**
`InlineToolbar` installs a new, never-ending relay `eval` on every button click
(`inline_toolbar.rs`, `relay_js`). Clicks therefore accumulate long-lived
evaluators. Do not copy that pattern. If you touch it, a single relay per
toolbar is the obvious shape. Either way, say in the request what you found.

### 2.4 Labels

The cell inputs get accessible labels now (RFC-048 §5.4): column header plus
row number, for example "Name, row 2". The full accessibility pass is slice 5.

## 3. Prohibited · **[Binding]**

- Row, column, alignment or Tidy operations. Those are slices 3–5.
- Rendering cells as rich text (§10 Q1 is A).
- Any change to bytes outside the edited cell.
- Running anything that initialises GTK, tao or a WebView on this machine.

## 4. Evidence and review request · **[Binding]**

- **Classification tests:** the cases that used to demote are now editable:
  - bold header;
  - italic;
  - strikethrough;
  - code;
  - inline HTML `<br>`;
  - a link.

  A disagreeing table is still an island.
- **Round-trip and edit tests on formatted cells**, each showing exactly which
  bytes changed. Include:
  - `` `a\|b` `` in code;
  - CRLF;
  - Japanese text.
- **Toolbar in a cell:** bold applied to a selection inside one cell changes
  only that cell. Headless, at the resolver level.
- **A WebView end-to-end check is not required in this slice.** RFC-048 §6
  puts it in slice 3 or 4.
- **Mutations, one each:**
  - restore the old demotion rule;
  - make the toolbar's offset table-relative instead of cell-relative;
  - drop the `|` escaping in toolbar results.
- A CHANGELOG entry under `[Unreleased]` `### Changed`. For example: "Tables
  whose cells use bold, italic, code or other inline formatting can now be
  edited in Form Mode, cell by cell; each cell shows its Markdown text."
- The local gates, with the Rust tests run with `CI` unset, and
  `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-048-slice-2-formatted-cells.md`.
Give the full tip hash and its base.
