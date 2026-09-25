# RFC-046 handoff — slice 1: the converter

**Governing RFC:** [RFC-046](../../accepted/RFC-046-paste-html-as-markdown.md) §5.3, §5.4, §6.2, §7, §8
**Slice:** 1 of 3 — the converter crate, headless. Slice 2 is the paste path; slice 3 is documentation.
**Baseline:** `main` at `6f6864a` or later. If `main` moves in a file this slice
touches, merge `origin/main` in; never rebase.
**Status:** inherited from RFC-046 (Accepted, 2026-09-16; upstream pin `mdka` `=2.5.1`, §6.2)
**Date:** 2026-09-25

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`: the shared
folder, a detached HEAD, no branch.

## 1. Purpose

This slice turns clipboard HTML into Markdown, or says why it declined, as a
**pure, headless crate**. Nothing in the app changes here: no paste handler,
no toast, no UI. Slice 2 wires it in.

## 2. The crate · **[Binding]**

- **`crates/bekoedit-paste`**, a new workspace member.
  - Its **dependencies are exactly `mdka = { version = "=2.5.1",
    default-features = false }` and `thiserror`** (RFC-046 §5.3, §6.2).
  - **Development dependencies may add `pulldown-cmark`** (workspace), for §5's
    structural test only.
- **Workspace wiring:**
  - add the crate to `members` and to `default-members`, so it is tested
    headlessly in every CI test job;
  - add a `bekoedit-paste = { version = "0.16.1", path = … }` workspace
    dependency;
  - **do not yet make `bekoedit` (the app) depend on it.** That is slice 2.
- **`release.yml`'s workspace check** names exactly five packages. Add
  `bekoedit-paste`. Otherwise the next release fails at validation.
  **Publishing it is not this slice's concern**, and needs the owner's approval
  (§9).
- **Re-measure RFC-046 §8** against `=2.5.1`, not 2.2.1:
  - which crates the lock file gains, by name;
  - that each builds with `cargo +1.88.0`.

  Report the list. If it differs materially from §8's six, say how.

## 3. The API · **[Advisory] shape, [Binding] behaviour**

A shape that fits slice 2, which you may change:

```rust
pub fn convert(html: &str, plain: &str, line_ending: LineEnding) -> Outcome;

pub enum Outcome {
    Converted { markdown: String, html_had_table: bool },
    Fallback(FallbackReason),   // slice 2 inserts `plain` and raises one toast
    Empty,                      // rule 4: insert `plain`, quietly
}
pub enum FallbackReason { TooLarge, Failed, TimedOut }
```

**Binding behaviour**, from RFC-046 §5.4 as amended:

1. **Too large:** the HTML is over **1 MiB**, measured **before** conversion,
   from one named constant. Result: `Fallback(TooLarge)`. `mdka` is not called.
2. **Failed:** `mdka` returns an error, or panics. **Catch the panic.** A paste
   must never take the app down. Result: `Fallback(Failed)`.
3. **Timed out:** conversion exceeds **2 s**, from one named constant. Result:
   `Fallback(TimedOut)`.
   - Mechanism **[Advisory]**. A worker thread with a bounded wait is enough.
     The stalled thread may run on, but its result must be dropped.
   - Say what happens to it.
4. **Empty:** the output is empty (or only whitespace) while `plain` is not.
   Result: `Empty`.
5. **`html_had_table`** is true when the HTML contains `<table`, ASCII
   case-insensitive. Slice 2 raises "pasted table as text: no Markdown table
   form" when this is true and the output has no GFM table (§5.4). The **output**
   check belongs in `bekoedit-markdown`: add `pub fn has_gfm_table(markdown:
   &str) -> bool` there, parsing with the document's own GFM options, with its
   own tests.

## 4. The guards · **[Binding]**

Applied to every `Converted` result:

- **No `data:` destination survives.** A `data:` image becomes its alt text,
  and an empty alt becomes a fixed marker (RFC-046 §10 Q4). Slice 2 supplies
  the localized text; here, a constant.
  - Mechanism **[Advisory]**. You have only `mdka` and `thiserror`. If this
    cannot be done robustly without a third dependency, or without
    string-parsing Markdown in a way you cannot prove, **stop and report**. Do
    not add a dependency on your own.
- **The output uses the target line ending**, everywhere.
- **No raw HTML except a bare `<br>` inside a table cell** (RFC-046 §5.3, as
  amended). This is enforced by §5's structural test, not by rewriting the
  output.

## 5. The fixture corpus · **[Binding]**

`crates/bekoedit-paste/tests/fixtures/`: pairs of `<name>.html` and
`<name>.md`, with a runner that converts each HTML file and compares it with the
Markdown, byte for byte.

- **Honest labelling.** Nobody here can capture real clipboard HTML: that needs
  a person at a real application. So every fixture in this slice is
  **hand-written**, and its name says so (`synthetic-…`). Real captures are
  future work, recorded in §8. Do not describe a synthetic fixture as a capture.
- **At minimum:**
  - our nine reproductions (`.git-exclude/upstream/mdka/send/2026-09-16-html-to-markdown-conversion-gaps.md`);
  - **upstream's 2.4.1 regression:** a table cell holding two sibling `<div>`s
    (`.git-exclude/upstream/mdka/receive/2026-09-24b-correction-take-2.5.1.md`);
  - a table with row headers (it falls back to paragraphs), and a nested table;
  - a table inside a blockquote and inside a list item, with `<br>` in a cell;
  - `Vec<i32>` in inline code and in a fenced block;
  - Google-Docs-shaped wrappers: `<b style="font-weight:normal" id="docs-internal-guid-…">`
    around blocks, and a `font-weight:700` span. The bold is lost, and that is
    expected per §6.2;
  - a definition list;
  - a `data:` image, with and without alt text;
  - CRLF and LF targets for the same input.
- **Invariants over the whole corpus:**
  - **the structural raw-HTML test.** Parse each output with the document's GFM
    options (the dev-dependency). Every `Html` and `InlineHtml` event must be
    exactly `<br>` and inside a `TableCell`. Anything else fails, naming the
    fixture and the event. A line scan is not acceptable;
  - no `data:` substring in any destination;
  - only the target line ending;
  - every fixture converts within the 2 s budget.
- **Size:** a generated 1 MiB input, just under the limit, converts. One byte
  over falls back as `TooLarge`. Report the release-build conversion time for
  1 MiB and for 5 MiB (the latter by calling `mdka` directly), and compare them
  with §5.3's 22 ms and 101 ms, measured on 2.2.1.

## 6. Upstream's claims, checked · **[Binding]**

`mdka` told us things that this slice can now check (§6.2):

- in `Minimal`, the only raw HTML is `<br>`, and only in cells;
- the 2.4.1 wrapper-in-cell bug is fixed;
- `<br>` outside a table becomes two trailing spaces;
- a literal `<` in prose is escaped.

Each gets a fixture. **Report any claim that does not hold.** That goes to
upstream in a letter, and slice 1 records it without working around it.

## 7. Prohibited · **[Binding]**

- Any change to `bekoedit-app` or its JavaScript. Slice 2 does those.
- Any dependency beyond §2's.
- Network access in tests.
- Running anything that initialises GTK, tao or a WebView on this machine.
- Weakening a fixture's expected output to make a run pass. If `mdka`'s output
  is wrong, the fixture records the **correct** expectation, fails, and is
  marked as a known upstream gap with a pointer. Do not quietly accept the wrong
  output.

## 8. What stays open after this slice

- **Real clipboard captures** from Firefox, Chromium, Google Docs, Word and
  LibreOffice, which a person must make. They are not a precondition for slice 2.
- **Publishing `bekoedit-paste` to crates.io** is the owner's decision, before
  the release that first contains it.

## 9. Evidence and review request · **[Binding]**

- The local gates, with the Rust tests run with `CI` unset, and
  `git diff --stat origin/main...HEAD | tail -1`.
- §2's dependency delta, §5's timings and §6's results.
- **Mutations**, each failing a named test:
  - the 1 MiB check removed;
  - the panic caught not at all;
  - the `data:` guard removed;
  - the line-ending normalisation removed;
  - an extra raw `<span>` let through by the structural test;
  - `html_had_table` made case-sensitive.

Review request: `.git-exclude/review-request/<date>-rfc-046-slice-1-the-converter.md`.
Give the full tip hash and its base.
