# RFC-031: Lexical Integration Decision

**Status:** Proposed (decision reached v0.4.0, 2026-06-07; no implementation required)  
**Milestone:** M8 (post-MVP)  
**Priority:** Post-MVP — decision reached as part of v0.4.0 research

---

## 1. Summary

This RFC evaluates whether the [Lexical](https://lexical.dev/) rich-text
framework should replace the current custom projection-based Form Mode
blocks, and records the project's decision.

**Decision: Remain on custom Form Mode projection blocks for the foreseeable
future. Lexical is not adopted for this codebase at this time.**

---

## 2. Evaluation

### What Lexical offers

Lexical is an extensible rich-text editor framework from Meta, designed
to be headless and React-friendly. Its node model (LexicalNode, ElementNode,
TextNode) can in principle represent structured Markdown blocks.

### Why it does not fit bekoedit's invariants

| Concern | Detail |
|---------|--------|
| **Ownership** | Lexical owns the document as its internal node tree. bekoedit's core invariant is that raw Markdown is canonical. The two are irreconcilable without a round-trip serializer — the same problem that motivated bekoedit's design. |
| **Round-trip fidelity** | Lexical's Markdown serializer ([@lexical/markdown](https://github.com/facebook/lexical)) regenerates the whole document from the node tree. This violates RFC-000 Invariant 4 (no whole-document rewrite from Form Mode). Preserving trivia (fence style, list marker, blank lines, reference links) would require a custom serializer of equivalent complexity to the current patch engine. |
| **Dependency footprint** | Lexical + required plugins (~120 kB min+gzip) would increase the bundle by ≈8× over the current custom block components. |
| **Source preservation tests** | The golden preservation test suite (RFC-000 §13) would fail with a naive Lexical integration because Lexical normalizes whitespace and inline structure. Making it pass would require the same source-range machinery already implemented. |
| **React dependency** | Lexical is designed for React. Dioxus Desktop renders via OS WebView with a Wasm/Dioxus component tree. Integrating a React library requires a separate React root, iframe, or postMessage bridge — substantial complexity for uncertain benefit. |

### When Lexical *would* fit

A future product built around a cloud-collaborative Markdown editor (outside
bekoedit's stated scope — see RFC-040) could adopt Lexical or ProseMirror as
the authoritative document model and implement CRDT-aware synchronization.
That is a different product category.

### Custom projection block approach: assessment

The current approach (RFC-015/016/017/018):
- Passes the golden source-preservation tests
- Operates as minimal byte-range patches
- Handles multibyte text, CRLF, style trivia
- Supports Raw Markdown Islands for unsafe structures
- Tested independently of the UI

Adding richer inline formatting (RFC-030) extends naturally via
`FormBlockEdit::ToggleInline` without touching the document ownership model.

---

## 3. Decision

**Do not adopt Lexical.** Continue with the custom Form Mode projection
approach. Invest in:

1. More `FormBlockEdit` variants for inline formatting (RFC-030 — shipped v0.4.0).
2. Simple table editing (RFC-027 — shipped v0.4.0).
3. Image card support (RFC-028 — shipped v0.4.0).

Revisit this decision only if profiling shows the custom patch engine cannot
handle document sizes or edit complexity that users actually encounter.

---

## 4. Open Questions

None. Decision is final for this product direction. Future evaluation of
alternative architectures (if any) belongs in a new RFC.
