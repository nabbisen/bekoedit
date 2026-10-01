# RFC-046 handoff — slice 2: the paste path

**Governing RFC:** [RFC-046](../../done/RFC-046-paste-html-as-markdown.md) §5.1, §5.2, §5.4, §5.5, §5.6, §7, §8
**Slice:** 2 of 3 — the paste handler, the bridge, the fallback notices, and plain paste. Slice 3 is documentation.
**Baseline:** `main` at the commit that adds this handoff, or later. If `main` moves in a file this slice
touches, merge `origin/main` in; never rebase.
**Status:** inherited from RFC-046 (Accepted; converter done in slice 1, pin `mdka` `=3.0.0`)
**Date:** 2026-09-26

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`: the shared
folder, a detached HEAD, no branch.

**This slice merges in two parts** (§2 and §3). Its central assumption can only
be tested in a real WebView, and CI sees a WebView only on a push to `main`.

## 1. Part 0: make the paste tests immune to a shared `target/` · **[Binding]**

`bekoedit-paste`'s tests find their files through `env!("CARGO_MANIFEST_DIR")`
(`src/tests.rs:389`, `tests/corpus.rs:32`). That value is fixed at compile time,
so a binary reused from a deleted worktree looks in a directory that no longer
exists. The task 037 merge report traced this.

**Read the variable at run time instead**, with `std::env::var("CARGO_MANIFEST_DIR")`,
which cargo sets when it runs a test. This is the first commit of part A.

**Also, as part B's first commit** (added 2026-09-30):

- the same change in `bekoedit-app`'s tests that use
  `env!("CARGO_MANIFEST_DIR")`: `link_guard/tests.rs`, `shell_focus/tests.rs`,
  `tests/rfc_042.rs`, `source_sync/host/tests.rs` and
  `tests/link_opener_stubs.rs`;
- `#[cfg(unix)]` on `paste_probe.rs`'s `XCLIP_STUB_BODY`, which the Windows
  build reports as unused.

## 2. Part A: prove the gating assumptions, then merge a probe · **[Binding]**

Before any product code, one **release-checks scenario** answers the questions
this slice rests on. It lands on `main` alone, with its first CI run read and
reported. **Stop there, and report.**

1. **A real paste.** Put HTML on the X clipboard (under Xvfb, in CI), with both
   `text/html` and `text/plain` targets. Focus the Text-mode editor, and send a
   real Ctrl+V through XTEST, as task 023 sends clicks.
   - Does a `paste` event reach a test listener on the editor, and does its
     `clipboardData` carry **both** flavours?
   - Mechanism **[Advisory]**: `xclip -selection clipboard -t text/html` is one
     way. Install it in CI only. **Never on this machine**, and never run it
     against the real display here.
2. **The plain-paste chord.** The same, with Ctrl+Shift+V. Does WebKitGTK
   dispatch a `paste` event for it at all? And do we see the keydown for the
   chord before it?
3. **Only if (1) fails:** can a script construct a `ClipboardEvent` whose
   `DataTransfer` carries `text/html`, dispatch it on the editor, and have it
   reach the handler? This is RFC-046 §7's fallback question.

The probe **records and reports; it does not assert product behaviour.** Its
outcome line reads PASSED once it has observed and reported, whatever the
answers are. The answers go in the merge report.

**If neither a real paste (1) nor a constructed one (3) reaches a handler, stop
and report.** Slice 2's end-to-end testing then needs a different design, and
that decision is mine.

## 3. Part B: the paste path

### 3.0 What part A established · **[Binding]**, added 2026-09-30

From run `36707615030`, with a GTK 4 owner serving both flavours, confirmed
from outside the WebView:

- **A real Ctrl+V carries both flavours to the page**, as a trusted `paste`
  event. The design stands, and **the end-to-end test uses the real clipboard**.
- **A real Ctrl+Shift+V carries only `text/plain`.** WebKitGTK strips the HTML
  for that chord itself, so the handler sees no `text/html` and lets
  CodeMirror's plain paste run.
- **WebKitGTK rewrites the HTML before the page sees it.** The owner served 84
  characters, and the page read **1,469**, beginning `<h1 style="caret-color:
  …`: every element carries computed inline styles. **This is the HTML a Linux
  paste will actually convert,** and it is exactly the shape where `mdka` drops
  bold carried by `style` (RFC-046 §6.2, item 4). So:
  - the end-to-end scenario **records the full HTML the page received**, as one
    line in the log;
  - that exact HTML becomes a fixture in `bekoedit-paste`'s corpus, named
    `webkitgtk-…`, **not** `synthetic-…`, as the first real serialisation in
    the corpus. Its expected Markdown is typed by hand;
  - **if the bold, or any heading or list structure, is lost in that
    conversion, report it before shipping.** Whether that is acceptable, or
    needs a pre-pass, or a letter to upstream, is my decision to make on
    evidence.

### 3.1 The handler · **[Binding]**

In `editor.js`'s existing `EditorView.domEventHandlers`, a `paste` handler
(RFC-046 §5.1):

- If the event carries `text/html` and plain paste was not requested (§3.5), it
  calls `preventDefault()`, keeps the `text/plain` flavour, records the
  selection and the editor identity, and sends a convert request (§3.2).
- Otherwise it does nothing, and CodeMirror's plain paste runs exactly as today.
- **An HTML flavour longer than the byte limit is decided locally, without
  crossing the bridge**, if its length already proves it. **[Advisory]**
  mechanism: a string longer than 1 MiB in UTF-16 code units is certainly over
  1 MiB in UTF-8. It becomes a `TooLarge` fallback.

### 3.2 The bridge · **[Binding]**

The request and reply types live in `crates/bekoedit-ui-contract/src/source_editor.rs`,
beside the existing contracts. No bridge schema bump (RFC-046 §10 Q3).

- **The request:** a correlation token, the HTML, and the plain flavour's
  length. It travels JS to Rust over the existing structured channel.
- **Rust calls `bekoedit_paste::convert`** off the UI thread.
- **The reply:** the token, and either Markdown or a fallback reason. It travels
  Rust to JS through `dispatch_request`, which since task 031 is a string literal
  read with `JSON.parse`. **No other path may carry converted content to the
  page.**
- **The app now depends on `bekoedit-paste`.** Report the release binary's size
  and a clean release build's time, before and after (RFC-046 §8).

### 3.3 Inserting the result · **[Binding]**

- **Convert with `LineEnding::Lf`, always.** The editor speaks editor form, and
  task 027's reconciliation gives every inserted line break the file's own
  ending. So a `Mixed` file needs no special mapping, and none may be added.
- **No trailing line break is inserted.** `mdka` ends its output with `\n`; strip
  trailing line breaks before insertion, so a pasted paragraph ends where the
  pasted text ends, like a plain paste.
- **One transaction, `userEvent: "input.paste"`**, replacing the selection (§5.2),
  so the paste is one undo step.
- **Positions are mapped.** If the user edits while conversion is in flight, the
  recorded selection is carried through every intervening change with
  CodeMirror's change mapping, and the result is inserted there.
- **Never into a different document.** If the editor's identity changed, or
  `adapter.isHeld()` is true when the reply arrives, the result is discarded.
  - **Amended here:** a discarded paste raises **one Warning notice** ("Paste
    not applied: the document changed before it was ready"). It is not dropped
    silently, following RFC-047's rule that a user's action is either done or
    reported.

### 3.4 The notices · **[Binding]**

Through the existing toast surface, localised in English and Japanese, **exactly
one per paste**, and only in these cases:

| Case | Kind | Text, in substance |
|---|---|---|
| `Fallback(TooLarge)` | Info | pasted as plain text: too large to convert |
| `Fallback(Failed)` | Info | pasted as plain text: could not convert |
| `Fallback(TimedOut)` | Info | pasted as plain text: conversion took too long |
| `html_had_table` and not `has_gfm_table(markdown)` | Info | pasted table as text: no Markdown table form |
| discarded (§3.3) | Warning | paste not applied: the document changed |
| `Empty`, or a successful conversion | none | — |

The `data:` image marker passes the localised text through
`convert_with_marker`.

### 3.5 Plain paste · **[Binding], subject to part A**

**Ctrl+Shift+V** (Cmd+Shift+V on macOS) pastes plain text (RFC-046 §5.5).

- **On Linux this already holds without any code** (§3.0): the chord's paste
  carries no `text/html`, so §3.1's handler does nothing.
- **Keep the one-shot flag anyway.** The chord's keydown sets it, and the next
  `paste` in the same editor consumes it and lets CodeMirror's plain paste run,
  even if `text/html` is present. WKWebView (macOS) and WebView2 (Windows) are
  untested here and may not strip the HTML. The flag makes the behaviour the
  same everywhere, and costs a few lines.

### 3.6 Tests · **[Binding]**

- **JS, against the existing fakes:**
  - HTML present or absent;
  - the plain-paste flag;
  - each fallback reply;
  - position mapping when the document changes mid-flight;
  - a discard on identity change, and on `isHeld`, each raising the notice;
  - exactly one notice per case in §3.4, and none on success or `Empty`;
  - no trailing line break.
- **Rust:** the request and reply round-trip; the handler calls the converter
  with `LineEnding::Lf` and the localised marker; the table notice appears only
  when §3.4 says so.
- **End to end, in release checks,** with part A's GTK 4 clipboard owner and
  real XTEST keys:
  - a real Ctrl+V of HTML with a heading, a list, **bold**, and a table must
    arrive as the expected Markdown;
  - a real Ctrl+Shift+V of the same clipboard must arrive as the plain flavour;
  - the bytes on disk after save are checked like task 026's;
  - the full received HTML is logged (§3.0).
- **Mutations**, each failing a named test:
  - no position mapping;
  - no discard;
  - a trailing newline kept;
  - the plain flag ignored;
  - a second notice raised for one paste;
  - `LineEnding::Crlf` passed.

### 3.7 CHANGELOG · **[Binding]**

Under `[Unreleased]`, *Added*: pasting formatted text from a browser or an
office app into Text mode inserts Markdown, and Ctrl+Shift+V pastes plain text.
State the known limits plainly:

- bold carried by styles, as Google Docs uses, arrives as plain text;
- tables with no Markdown form become paragraphs, with a notice.

## 4. Prohibited · **[Binding]**

- **Form Mode** (RFC-046 §4). This slice touches only the Text-mode editor.
- **Any route for converted content other than §3.2's.**
- A silent discard, or a silent fallback other than `Empty`.
- Installing `xclip`, or any tool, on this machine. Running anything that
  initialises GTK, tao or a WebView here, or the opener test with `CI` set.

## 5. Evidence and review requests · **[Binding]**

- **Part A's request, then its merge report,** with the answers to §2's
  questions, quoted from the CI log.
- **Part B's request:**
  - the local gates, with the Rust tests run with `CI` unset;
  - `git diff --stat origin/main...HEAD | tail -1`;
  - the binary size and build time from §3.2;
  - the mutations.

Review requests:

- `.git-exclude/review-request/<date>-rfc-046-slice-2a-paste-probe.md`;
- `.git-exclude/review-request/<date>-rfc-046-slice-2b-the-paste-path.md`.

Give the full tip hash and its base.
