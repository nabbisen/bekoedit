# RFC-046: Paste HTML as Markdown

**Project:** bekoedit
**Status:** Proposed — drafted 2026-09-16 at the project owner's request. Not
yet approved for implementation. Implementation is deliberately scheduled
**after RFC-044's remaining slices** (slice 2 and slice 3), per the owner's
decision of 2026-09-16.
**Track:** Editing
**Priority:** Medium — a common authoring path that currently loses all structure
**Date:** 2026-09-16
**Related RFCs:** [RFC-011](../done/RFC-011-text-mode-with-codemirror-6.md), [RFC-015](../done/RFC-015-sourcepatch-engine-and-source-preserving-mutation.md), [RFC-016](../done/RFC-016-form-mode-mvp-surface-and-safe-editable-blocks.md), [RFC-017](../done/RFC-017-raw-markdown-islands.md), [RFC-038](../done/RFC-038-advanced-markdown-extension-policy.md), [RFC-041](../done/RFC-041-source-editor-lifecycle-and-synchronization-controller.md), [RFC-044](../accepted/RFC-044-shell-behaviour-regression-coverage.md)
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

1. The HTML exceeds the size cap. Proposed default: 1 MiB (§10 Q2).
2. Conversion fails, or takes longer than a bounded time (proposed 2 s).
3. The HTML contains `<table>`. `mdka` 2.2.1 flattens every table — `AB12` from a
   two-by-two table — and bekoedit edits GFM tables (RFC-027). A tab-separated plain
   paste is better than a destroyed table. This rule is removed once `mdka` emits
   GFM tables (§6, request 1).
4. Conversion produces empty output while the plain flavour is not empty.

A fallback is silent: the user sees an ordinary plain paste.

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
Whether this bumps the bridge schema version is §10 Q3.

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
bekoedit. The draft with reproductions is kept at
`.git-exclude/governance/2026-09-16-mdka-upstream-requests.md`. bekoedit does not
block on them: §5.4's fallbacks and §5.3's guards cover the gaps, and each fix
arrives here as a version bump with updated fixture expectations.

## 7. Testing

- **Rust, headless.** A fixture corpus of real clipboard HTML from browsers, Google
  Docs, Word and LibreOffice, with expected Markdown. Every §5.4 fallback rule has
  its own case. Invariants run over the whole corpus: no raw HTML, no `data:` URI,
  line endings normalised.
- **JavaScript.** The paste handler against the existing shared fakes under
  `crates/bekoedit-app/js/test/`: HTML present or absent, the plain-paste flag, a
  fallback reply, position mapping when the document changes mid-flight, and
  discard when the editor identity changes.
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
   discard rules, and §5.5's plain-paste chord. Leads with the §7 gating
   assumption.
3. **Surfacing.** Documentation, the manual walkthrough items, and §10 Q1 if a
   setting is chosen.

Scheduled after RFC-044's slices 2 and 3.

## 10. Open questions

1. **A setting to turn conversion off?** Ctrl+Shift+V already gives plain paste per
   occasion. A persistent toggle adds a settings key and UI. Proposed: no setting
   in the first release; revisit on feedback.
2. **Size cap.** Proposed 1 MiB of HTML. Conversion is fast enough for more (5 MB in
   101 ms); the binding cost is moving the string across the WebView bridge.
3. **Bridge schema version.** Do new message types require a version bump under
   RFC-041's protocol, or are additive messages compatible?
4. **`data:` images.** Replace with alt text (proposed), drop entirely, or keep a
   short placeholder image target in place of the URI?
5. **Tables before `mdka` supports them.** Wait for the upstream fix (proposed), or
   convert simple tables in bekoedit as an interim step?
6. **Pasted `javascript:` links.** `mdka` keeps `href` verbatim. Typed Markdown can
   already contain such links, so this is not new exposure, but a paste makes it
   easier to acquire one unknowingly. Does Preview mode already neutralise them?
