# RFC-046 handoff — slice 3: surfacing

**Governing RFC:** [RFC-046](../../accepted/RFC-046-paste-html-as-markdown.md) §5.4, §5.5, §6.4, §7, §9
**Slice:** 3 of 3 — documentation. No code.
**Baseline:** `main` at `26ce141` or later. If `main` moves in a file this slice
touches, merge `origin/main` in; never rebase.
**Status:** inherited from RFC-046 (Accepted; slices 1 and 2 done)
**Date:** 2026-10-01

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`: the shared
folder, a detached HEAD, no branch.

## 1. Purpose

The paste path is built and tested, but a user reading the docs would not know
it exists, what it does, or what its notices mean. This slice says so, plainly
and accurately. **Every statement must match the code on `main`**, and be checked
against it, not written from memory of the handoffs.

## 2. Required · **[Binding]**

### 2.1 `docs/src/editing-modes.md`: pasting formatted text

A section a user can act on. Cover, at least:

- **What happens on Ctrl+V** (Cmd+V on macOS) in Text Mode, when the clipboard
  holds formatted content: it becomes Markdown, as one undo step. **What
  converts:** headings, paragraphs, bold and italic (including bold set by
  styles, as Google Docs uses), links, lists, task lists, strikethrough, code,
  and tables.
- **Ctrl+Shift+V** (Cmd+Shift+V) pastes plain text, unconverted.
- **When it pastes plain text instead, and says so:** too large, could not
  convert, and took too long. Name the limits from the code: 1 MiB of HTML, and
  2 seconds.
- **A table with no Markdown form** (row headers, two header rows, a nested
  table, or a caption): its text arrives as paragraphs, with a notice.
- **Images embedded in the page** (`data:`) arrive as their description, or a
  short marker if they have none.
- **A paste that arrives after you switched documents** is not applied, and a
  notice says so.
- **Your file is never changed beyond the pasted text**, and keeps its own line
  endings. Link to `source-preservation.md`.
- **Form Mode:** say what a paste does there. Check the behaviour in the code;
  do not assume it.
- **The known limit, plainly:** bold declared on a block around a heading also
  makes the heading bold.

**The notices:** quote their English text exactly as `i18n.rs` has it, so a user
can match what they see. Mention that a Japanese text exists, and do not
translate it here.

### 2.2 `docs/src/editing-modes.md`, Preview Mode: links

Task 030 and 032's behaviour is user-facing and undocumented:

- only `http(s):` and `mailto:` links open, in the browser or mail client;
- a relative link (`other.md`) or a network path does not open, and a notice
  says why;
- following links between notes is not supported yet;
- a `#` link stays in the page;
- links with other schemes, such as `javascript:`, are shown as plain text;
- a `<br>` inside a line, for example in a table cell, shows as a line break.

### 2.3 `docs/src/architecture.md`

"Five crates" becomes six. Add `bekoedit-paste`:

- its one job;
- **its dependency boundary**: `mdka` (exact pin) and `thiserror` only, so that
  `bekoedit-markdown` stays free of an HTML parser;
- that the app calls it off the UI thread.

Keep the style of the existing crate descriptions.

### 2.4 `README.md`, *Features*

One bullet, in the style of its neighbours.

## 3. Release walkthrough items: propose them, do not edit the checklist · **[Binding]**

**Do not edit `docs/src/manual-release-checklist.md`.** The owner maintains it.

Instead, put **proposed walkthrough items** for the paste feature in the review
request:

- what a person checks by hand that CI cannot: a real paste from Firefox,
  Chromium, Google Docs and LibreOffice, on each OS;
- the expected results, including the known limit.

I put them into the 0.17.0 release evidence.

## 4. Prohibited · **[Binding]**

- Any code change.
- `docs/src/manual-release-checklist.md`.
- Describing behaviour you did not check against the code.
- A CHANGELOG change. The paste entry is already accurate, as of task 038.

## 5. Evidence and review request · **[Binding]**

- **For every factual claim in the new text, where in the code it is true.**
  File and line, as a list in the request: limits, notice texts, Form Mode
  behaviour, and link rules.
- `scripts/check-rfcs.sh` and `scripts/check-changelog.sh`.
- If the book builds in CI, it builds. If you add a page, update `SUMMARY.md`.
- `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-046-slice-3-surfacing.md`.
Give the full tip hash and its base.
