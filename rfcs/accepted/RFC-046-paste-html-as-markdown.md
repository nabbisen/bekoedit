# RFC-046: Paste HTML as Markdown

**Project:** bekoedit
**Status:** Accepted — approved for implementation by the project owner on
2026-09-16, the day it was drafted. Implementation is deliberately scheduled
**after RFC-044's remaining slices** (slice 2 and slice 3), per the owner's
decision of the same date; acceptance authorises the design, not an immediate
start.
**Track:** Editing
**Priority:** Medium — a common authoring path that currently loses all structure
**Date:** 2026-09-16
**Related RFCs:** [RFC-011](../done/RFC-011-text-mode-with-codemirror-6.md), [RFC-015](../done/RFC-015-sourcepatch-engine-and-source-preserving-mutation.md), [RFC-016](../done/RFC-016-form-mode-mvp-surface-and-safe-editable-blocks.md), [RFC-017](../done/RFC-017-raw-markdown-islands.md), [RFC-038](../done/RFC-038-advanced-markdown-extension-policy.md), [RFC-041](../done/RFC-041-source-editor-lifecycle-and-synchronization-controller.md), [RFC-044](../done/RFC-044-shell-behaviour-regression-coverage.md)
**Upstream:** [`mdka`](https://github.com/nabbisen/mdka-rs), maintained by the project owner

---

## 1. Summary

Pasting formatted content copied from a browser, Google Docs, Word or LibreOffice
into Text or Split mode should produce Markdown — headings, lists, emphasis,
links, code — rather than flattened plain text. Conversion is done by `mdka`, an
HTML-to-Markdown converter that shares this project's maintainer, MSRV and
license. Where `mdka` cannot yet produce faithful Markdown, bekoedit falls back to
today's plain-text paste, and the gap is raised with the `mdka` project instead
of being worked around here.

## 2. Motivation

Today a paste into the source editor takes the clipboard's `text/plain` flavour.
Everything structural is lost: a copied web article arrives as unmarked
paragraphs, a list loses its bullets, a link loses its target. Rebuilding that by
hand is the most repetitive task a Markdown author does.

The clipboard usually already carries the structure, as `text/html`. Converting it
is a well-understood problem, and this project's maintainer also maintains a
converter for exactly that. The cost is measured (§8), and small.

## 3. Goals

- An HTML paste into Text or Split mode inserts readable, CommonMark/GFM-compatible
  Markdown.
- The paste is an ordinary edit: only the inserted bytes change, it is one undo
  step, and it reaches Rust through the existing synchronisation path (§5.2).
- Pasting plain text stays one keystroke away (§5.5).
- Where conversion would lose content, the user gets today's plain-text paste
  instead of damaged Markdown (§5.4).

## 4. Non-goals

- **Form Mode.** Its fields hold the text of a single block (RFC-016). Converting
  pasted HTML into multi-block Markdown there would bypass the safe-editable-block
  model. Form Mode keeps plain-text paste.
- **Importing HTML files** or web pages as documents. A possible later RFC; it
  shares the converter but not the editor path.
- **Copying out as HTML.** This RFC covers the inbound direction only.
- **Images and assets.** A pasted image is not saved into the workspace. `data:`
  URIs are handled by §5.3's guard, not imported.
- **Changing the raw-HTML policy** (RFC-017, RFC-038). Converted output must not
  introduce raw HTML at all (§5.3).

## 5. Design

### 5.1 Where the paste is intercepted

In the CodeMirror 6 source editor (`crates/bekoedit-app/js/src/editor.js`),
bundled with `@codemirror/view` 6.43.0.

- **Not `EditorView.clipboardInputFilter`.** It is present in the bundle, but it
  receives the already-extracted `text/plain` string, never the HTML flavour.
- **A `paste` handler in the existing `EditorView.domEventHandlers`.** When the
  event's `clipboardData` carries `text/html` and plain paste was not requested
  (§5.5), the handler calls `preventDefault()`, keeps the `text/plain` flavour for
  fallback, and asks Rust to convert (§5.6). Otherwise it does nothing, and
  CodeMirror's built-in plain paste runs exactly as today.

### 5.2 The inserted text is an ordinary edit · **source preservation**

The converted Markdown is inserted as **one CodeMirror transaction** at the
selection, with `userEvent: "input.paste"`, so it is one undo step and replaces
the selection like any paste.

From there nothing is new. The existing `updateListener` schedules
`publishChange`, which sends the document to Rust through RFC-041's lifecycle
adapter — the same path as typing. No other bytes in the document change.

Two constraints, because conversion is asynchronous:

- **Positions are mapped.** Edits the user makes while conversion is in flight
  move the insertion point through CodeMirror's change mapping. Nothing is
  inserted at a stale offset.
- **Never into a different document.** If the editor was destroyed, remounted, or
  replaced (RFC-041 identity changed) before the reply arrives, the result is
  discarded. If `adapter.isHeld()` is true at insertion time, the existing
  transaction filter would drop the change anyway; the paste is discarded rather
  than retried on a timer.

### 5.3 Conversion in Rust

- **Crate placement.** A new leaf crate — working name `bekoedit-paste` — depending
  on `mdka` and `thiserror` only. `bekoedit-markdown` stays free of an HTML parser,
  and the logic stays testable headlessly (`bekoedit-app` is excluded from default
  builds).
- **`mdka` with `default-features = false`.** Default features pull in `rayon`,
  and `jemalloc` is opt-in. Neither is needed for clipboard-sized input: 1 MB of
  HTML converts in 22 ms and 5 MB in 101 ms (release build, measured).
- **`ConversionMode::Minimal`**, chosen on evidence (§6). Balanced and Semantic
  emit raw `<a id="…"></a>` anchors inside headings. Minimal does not, and it also
  drops page chrome (`nav`, `footer`) that a sloppy selection may include.
- **Guards applied by bekoedit after conversion:**
  - `data:` URI images are replaced by their alt text. A pasted screenshot must not
    put megabytes of base64 into the document.
  - Output is normalised to the document's line ending.
  - The output must contain no raw HTML. A test enforces it over the fixture
    corpus (§7).

### 5.4 When bekoedit falls back to plain text · **[Binding]**

The `text/plain` flavour kept in §5.1 is inserted instead, through the same
transaction, when any of these is true:

1. The HTML flavour exceeds **1 MiB**, measured before conversion (§10 Q2), from
   a single named constant rather than a literal at each site.
2. Conversion fails, or exceeds a **2 s** budget — settled here rather than left
   "proposed". A 1 MiB paste converts in 22 ms, so 2 s is two orders of magnitude
   of headroom: it can only be reached by a genuine stall, which is precisely when
   the plain flavour should win.
3. The HTML contains `<table>`. `mdka` 2.2.1 flattens every table — `AB12` from a
   two-by-two table — and bekoedit edits GFM tables (RFC-027). A tab-separated plain
   paste is better than a destroyed table. This rule is removed once `mdka` emits
   GFM tables (§6, request 1).
4. Conversion produces empty output while the plain flavour is not empty.

**A fallback is announced, not silent** · amended 2026-09-16, on review.

The draft said a fallback is silent and the user simply sees an ordinary plain
paste. That is wrong for *this* project. Architectural invariant 7 is that unsafe
regions are **never silently normalized**, and a silent fallback is precisely a
silent normalization: a pasted table arrives as tab-separated text with nothing to
say its structure was dropped. An editor whose entire identity is source
preservation should not quietly lose content and look as though it did not.

Each fallback therefore raises exactly one non-modal toast through the existing
`components/toast.rs` surface, naming the reason in a short phrase — "pasted as
plain text: table", "…: too large", "…: conversion timed out". It never blocks,
never asks a question, and never needs dismissing. It is the smallest honest
signal that the paste took the other branch.

Rule 4 (empty output) is the one case that may stay quiet, because an empty
conversion of empty-looking HTML is not a loss the user needs told about.

### 5.5 Pasting plain text on purpose

**Ctrl+Shift+V** (Cmd+Shift+V on macOS) pastes plain text. It is not bound today:
`crates/bekoedit-app/assets/shortcuts.js` handles only Mod+S, Mod+1–4 and Mod+B.

Mechanism **[Advisory]**: a keydown on that chord sets a one-shot flag, and the
next `paste` event in the same editor consumes it and lets CodeMirror's plain
paste run. Whether WebKitGTK dispatches a `paste` event for this chord must be
verified in slice 2.

### 5.6 The conversion message

A request/response pair crosses the WebView boundary: the request carries a
correlation token, the HTML and the plain flavour's length; the reply carries the
token and either Markdown or a fallback reason. The types live beside the existing
source-editor contracts in `crates/bekoedit-ui-contract/src/source_editor.rs`.
No bridge schema bump is needed (§10 Q3).

The two directions are not symmetric, and the asymmetry decides a precondition:

- **JS → Rust** (the HTML) travels `relay(JSON.stringify(...))` → `dioxus.send` →
  `relay.recv::<serde_json::Value>()`. A structured channel. Nothing to add.
- **Rust → JS** (the Markdown) travels `dispatch_request`, which serializes with
  `serde_json` and **interpolates the result into JavaScript source text** —
  `const request = {payload};` at `source_sync/host.rs:363` — which
  `document::eval` then evaluates.

That second path is where converted, externally-authored content would arrive.
It is sound on every engine bekoedit supports, but only because ES2019 legalised
U+2028/U+2029 inside string literals; `serde_json` emits both literally, as
measured. Nothing in the repository asserts or tests that dependency.

**Precondition · [Binding].** Before slice 2 ships the paste path, the Rust → JS
payload must be emitted as an escaped JavaScript *string literal* consumed by the
`JSON.parse` branch that `js/src/editor.js:159` already has, with U+2028/U+2029
escaped explicitly. This is a pre-existing transport correction, **not** part of
this RFC's scope — it is recorded in
`.git-exclude/governance/2026-09-16-bridge-payload-js-interpolation-finding.md`.
RFC-046 does not fix it and must not ship untrusted content over it first. It
also removes the payload from the JavaScript parser, which is what actually
bounds §10 Q2.

## 6. `mdka` today — evidence, and requests to its project

Measured against `mdka` 2.2.1, `default-features = false`, in `Minimal` mode unless
noted, and rendered with `pulldown-cmark` 0.13 where correctness was in doubt.

| Input | Output | Verdict |
|---|---|---|
| Headings, bold, italic, links with titles | `## Intro … **bold** … *it* [link](https://e.x "t")` | Correct |
| Nested lists, clipboard `StartFragment` markers | `- a\n  - b\n- c` | Correct |
| `<script>`, `<style>`, `<iframe>` | removed | Correct (all modes) |
| Word markup (`MsoNormal`, `<o:p>`) | `**Bold** text\n\nnext` | Correct |
| Inline code; `<ol start="3">` | `` `x = 1` ``; `3. three` | Correct |
| Blockquote with `<br>` | `> q1  \n> q2` | Correct; trailing-space hard break |
| Balanced: heading with `id` | `## <a id="intro"></a>Intro` | Raw HTML — why Minimal |
| Any `<table>`, with or without `thead` | `AB12` | **Structure lost** |
| Google Docs `<b style="font-weight:normal">` around blocks | `**\n\npara one\n\npara two\n\n**` | **Broken Markdown** |
| `<span style="font-weight:700">` | plain text | Emphasis lost |
| `<del>`, `<s>` | plain text | Strikethrough lost |
| `<li><input type="checkbox" checked>` | `-  done` | Task state lost |
| Text `1. not a list` | `\1. not a list` | **Visible backslash** — CommonMark cannot escape a digit; correct is `1\.` |
| `data:` image | an image whose target is the full base64 `data:` URI | Passes through — §5.3 guard |

Each gap becomes a request to the `mdka` project rather than a workaround in
bekoedit. The letter carrying them, with reproductions, lives under
`.git-exclude/upstream/mdka/send/` — inside `draft/` until it is actually sent,
and moved up out of it once it has been. The folder is the state, as it is for
RFCs, so this reference is deliberately to the directory rather than to a filename
that is meant to move. bekoedit does not block on any of it: §5.4's fallbacks and
§5.3's guards cover the gaps, and each fix arrives here as a version bump with
updated fixture expectations.

## 7. Testing

- **Rust, headless.** A fixture corpus of real clipboard HTML from browsers, Google
  Docs, Word and LibreOffice, with expected Markdown. Every §5.4 fallback rule has
  its own case. Invariants run over the whole corpus: no raw HTML, no `data:` URI,
  line endings normalised.
- **JavaScript.** The paste handler against the existing shared fakes under
  `crates/bekoedit-app/js/test/`: HTML present or absent, the plain-paste flag, a
  fallback reply, position mapping when the document changes mid-flight, and
  discard when the editor identity changes. Also §5.4's signalling, since it is
  now user-visible behaviour rather than an internal branch: a fallback raises
  **exactly one** toast naming its reason, rule 4 raises none, and a successful
  conversion raises none either.
- **WebView, in RFC-044's second run — with a gating assumption proven first.**
  Unlike Tab (RFC-044 §8 A.1), a paste does not depend on a browser default action
  here: the handler calls `preventDefault()` and inserts the text itself. It is
  testable **if** WebKitGTK lets a script construct a `ClipboardEvent` carrying a
  `DataTransfer` with `text/html`. Slice 2 proves that first, in the smallest
  form, and stops and reports if it does not hold.
- **Manual walkthrough.** A real paste from Firefox, Chromium, Google Docs and
  LibreOffice, plus Ctrl+Shift+V. A trusted paste from a real application is
  something only a person can perform.

## 8. Cost

Measured by resolving `mdka` 2.2.1 against this workspace's `Cargo.lock`:

- 49 crates in `mdka`'s dependency closure; **43 are already present**, because
  Dioxus's WebView stack brings `html5ever`, `markup5ever`, `selectors`,
  `cssparser` and `string_cache`.
- **6 new crates:** `scraper`, `ego-tree`, `web_atoms`, `derive_more-impl`,
  `getopts`, `unicode-width`.
- `rust-version` 1.88, matching this workspace. License Apache-2.0, matching.
- It builds with `cargo +1.88.0` and `default-features = false`.

## 9. Slices

1. **The converter.** The new crate, Minimal mode, §5.3 guards, §5.4 fallback
   rules, and the fixture corpus. Headless; no UI change.
2. **The paste path.** §5.1's handler, the §5.6 message, §5.2's mapping and
   discard rules, §5.5's plain-paste chord, and §5.4's fallback toasts. Leads with
   the §7 gating assumption.

   **Two preconditions, neither of them this RFC's work.** Both are pre-existing
   defects found while reviewing it, both are small, and both must land before
   this slice starts carrying externally-authored content:

   - the Preview link-scheme filter —
     `.git-exclude/governance/2026-09-16-preview-link-scheme-finding.md`;
   - the bridge payload encoding (§5.6) —
     `.git-exclude/governance/2026-09-16-bridge-payload-js-interpolation-finding.md`.

   Slice 1 is unaffected and may proceed ahead of either.
3. **Surfacing.** Documentation and the manual walkthrough items. No settings
   work: §10 Q1 is settled as "no toggle".

Scheduled after RFC-044's slices 2 and 3.

## 10. Questions — all six answered 2026-09-16

Left in place with their answers rather than deleted, so the reasoning stays
readable next to what it decided. Each answer records its revisit trigger where
one exists.

1. **A setting to turn conversion off?** ~~Proposed: no setting in the first
   release.~~ **Answered 2026-09-16 — confirmed, no setting.**

   First, a correction to the draft's own reasoning: I priced this as "adds a
   settings key and UI", and that was wrong. `components/settings_screen.rs`
   already exists, and `AppSettings` is a flat `serde` struct where every field
   carries `#[serde(default)]`, so a new toggle is roughly three lines plus one
   row in a screen that is already built, and it is backward-compatible with
   existing settings files. **The cost is not the reason to decline it**, and a
   decision resting on a cost I had overstated would not have survived scrutiny.

   The real reason is that the two arguments *for* a toggle both dissolved. One
   was unpredictability — but §5.4 now announces every fallback, so the user can
   always see which branch a paste took. The other was the silent change of a
   long-standing default, which the same amendment fixes. What remains is a
   persistent mode duplicating Ctrl+Shift+V, the standard per-occasion gesture
   users already expect. Two ways to express one intent is the kind of knob that
   makes a program feel unfinished rather than configurable.

   **Revisit trigger**, so this is a decision and not a refusal: feedback showing
   users reaching for plain paste *repeatedly within a session* — that is a
   sustained preference, which a per-occasion chord genuinely serves badly, and
   it would justify the toggle on evidence.

2. **Size cap.** **Answered 2026-09-16 — 1 MiB, applied to the HTML flavour,
   before conversion.**

   The draft's stated basis was wrong and worth correcting: "the binding cost is
   moving the string across the WebView bridge" is too vague, and §5.6 now says
   what the cost actually is. The payload is interpolated into JavaScript
   **source** and parsed by the JS engine, so a large paste is a large *program*
   to tokenize, not merely bytes to copy. Conversion itself is nowhere near the
   constraint: 1 MB converts in 22 ms and 5 MB in 101 ms.

   1 MiB is the right number for two reasons that are independent of that defect.
   Clipboard HTML runs an order of magnitude larger than the Markdown it yields —
   Word and Google Docs markup especially — so 1 MiB of HTML is already a
   document-sized paste rather than a paragraph. And it sits coherently below the
   2 MB this project *already* calls large, in `bekoedit-fs`'s
   `large_file_warn_bytes` default: bekoedit should not silently accept more in a
   single paste than it would open a whole file without warning about.

   Measure the HTML flavour and reject **before** converting, so the guard costs a
   length check. The cap is one named constant shared with §5.4 rule 1, not a
   literal at each site, and it is **not** user-configurable — per Q1, this is not
   a program that grows a knob for every threshold.
3. **Bridge schema version.** ~~Do new message types require a version bump under
   RFC-041's protocol, or are additive messages compatible?~~ **Answered
   2026-09-16 — no bump needed.** The JS side is not shipped separately: `source_sync/host.rs`
   embeds `assets/editor-bundle.js` with `include_str!`, and the `tests.rs` bundle
   check pins that artifact to `js/src/lifecycle.js` and `js/src/editor.js`. Both
   halves of the protocol therefore leave the build in one binary, and the version
   skew `requireVersion` guards against cannot arise in a released bekoedit — the
   guard is a stale-bundle tripwire, not feature negotiation. RFC-046 adds its
   messages at `BRIDGE_SCHEMA_VERSION = 2`.

   **One constraint this imposes on the implementer.** `lifecycle.js`'s request
   dispatch ends in `default: return false`, so an unrecognised `type` is dropped
   in silence. The paste path must not rely on that: a conversion request that the
   JS side does not handle has to surface as a visible failure, not a paste that
   quietly does nothing. Give the new messages an explicit failure reply in the
   same style as `emitFailure`'s existing cases.
4. **`data:` images.** **Answered 2026-09-16 — replace with the alt text**, and
   with a stronger reason than the draft's "a screenshot must not put megabytes of
   base64 into the document".

   Form Mode makes it concrete. `bekoedit-markdown`'s `Block::Image { alt, src }`
   (RFC-028) renders a preview card with an **editable path field**, and
   `ReplaceImage` writes it back. A multi-megabyte base64 `src` does not merely
   bloat the file: it lands in a text input the user is expected to read and edit,
   in a mode this RFC does not otherwise touch. The document is also saved that
   way. Neither is acceptable, and neither is fixed by a shorter placeholder.

   The rule: an image whose destination is a `data:` URI becomes its alt text as
   plain text. Where the alt text is **empty**, insert a short localized marker
   rather than nothing — silently deleting content is the same mistake §5.4 was
   just amended to stop making, and a marker is what lets the user see that
   something was dropped and go fetch it.

   Note this decides what to **insert**, not how to alter an existing document, so
   the source-preservation invariant is not engaged: the bytes on disk never held
   the `data:` URI in the first place.

   This is deliberately *not* the same rule as the `data:` question in
   `.git-exclude/governance/2026-09-16-preview-link-scheme-finding.md`, though the
   two must be decided together. Here the concern is size, and the answer is
   "never insert". There the concern is safety, and the answer will be about
   rendering — an inline `data:image` is largely harmless to render, while
   `data:text/html` is not. Same code point, different questions.

5. **Tables before `mdka` supports them.** **Answered 2026-09-16 — wait for
   upstream. No interim converter in bekoedit.**

   Five reasons, in the order they weigh. §1 already commits this project to
   raising gaps with `mdka` rather than working around them here, and a
   contradiction in the first release would make that policy decorative. The owner
   maintains `mdka`, so the work is spent once and every `mdka` consumer gets it,
   instead of twice with two behaviours to reconcile. An interim converter would
   need its own HTML table parsing inside `bekoedit-paste`, pushing that leaf crate
   past `mdka` + `thiserror` and duplicating exactly the dependency §5.3 keeps out
   of `bekoedit-markdown`. The §5.4 rule-3 fallback is non-destructive and, after
   the amendment, announced. And bekoedit already models simple GFM tables
   (RFC-027 `Table { headers, rows, col_count }`), so the day `mdka` emits them the
   integration is deleting rule 3 and updating fixtures.

   **One honest weakness, recorded rather than buried.** Rule 3 is coarse: a long
   article containing one small table falls back *entirely*, losing the headings
   and lists that would have converted perfectly. The finer alternative — convert
   the non-table content and admit each table as a Raw Markdown Island (RFC-017) —
   is available and fits the architecture, but it is real complexity for a case
   that disappears the moment upstream lands. Keep the coarse rule for slice 1.
   **Revisit trigger:** upstream table support not released by the time slice 1 is
   otherwise ready to ship.
6. **Pasted `javascript:` links.** `mdka` keeps `href` verbatim. Typed Markdown can
   already contain such links, so this is not new exposure, but a paste makes it
   easier to acquire one unknowingly. ~~Does Preview mode already neutralise them?~~
   **Answered 2026-09-16 — no, it does not.** Measured against
   `render_preview_html`. A Markdown link whose destination is
   `javascript:alert(1)` renders as `<a href="javascript:alert(1)">`, and the
   case variant `JaVaScRiPt:` survives unchanged — so any filter has to compare
   case-insensitively. What *is* neutralised is raw HTML: an `<a>` element
   written literally in the document is escaped to text. The gap is therefore
   specifically Markdown link and image syntax, whose destination
   `pulldown-cmark` passes through untouched. `dioxus_config` sets no CSP and installs no navigation
   handler, and Preview injects through `dangerous_inner_html`.

   Architectural invariant 10's body ("converts every raw HTML event to escaped
   text") is accurate; its headline ("Preview never executes document HTML") is
   broader than what the code delivers.

   **This is pre-existing and not caused by RFC-046** — it is reachable today by
   typing the link by hand. RFC-046 changes only how easily one is acquired
   without noticing, since the rendered anchor text can read as anything. Treating
   it is therefore not a precondition for this RFC, and equally must not be
   deferred *to* this RFC. Recorded separately as a finding in
   `.git-exclude/governance/2026-09-16-preview-link-scheme-finding.md`, which is
   where the scope and the fix belong. Whatever scheme filter lands there should
   be in `render_preview_html`, so Preview, and any later export, inherit it
   without RFC-046 carrying a rule of its own.
