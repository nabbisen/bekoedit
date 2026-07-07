# bekoedit Architecture

This document restates the architectural invariants of
`rfcs/proposed/RFC-000-project-charter-and-architectural-invariants.md`
(per its §8, these are repeated here normatively) and maps them to the
code.

## Invariants

1. **Raw Markdown text is the canonical document.** It lives in
   `bekoedit_core::DocumentSession::canonical_text`. Nothing else is the
   document.
2. **Every visual surface is a projection.** `MarkdownIndex`,
   `FormProjection`, the preview HTML, and the file tree are derived,
   disposable, and rebuilt after every accepted mutation. Projections are
   never edited in place and never serialized back into Markdown.
3. **Rust owns source mutation and the filesystem.** The WebView UI sends
   *intent* (`bekoedit_ui_contract::UiToCoreCommand`); Rust validates,
   resolves, and mutates. JavaScript never owns authoritative byte ranges
   and never receives filesystem handles.
4. **Mutations are minimal source patches.** Form Mode semantic commands
   (`FormBlockEdit`) resolve to `SourcePatch` values that replace exactly
   the targeted bytes (`bekoedit_markdown::form::resolve_form_edit`).
   Whole-document rewrite from Form Mode is impossible by construction;
   Text Mode alone may replace the full text, because Text Mode *is* the
   raw source editor (RFC-011 snapshot strategy).
5. **All byte ranges are UTF-8 validated.** `ByteRange::validate` checks
   bounds and char boundaries before any mutation; a patch that would
   split a multibyte character is rejected, never "fixed up".
6. **Revision-scoped identity guards staleness.** Every mutation
   increments the document revision. Commands carry `base_revision` and a
   `BlockId` (revision + ordinal + kind + content/context fingerprint);
   mismatches are structured, recoverable rejections — the UI refreshes
   its projection and retries.
7. **Unsafe regions become Raw Markdown Islands.** Front matter, HTML
   blocks, tables, math, nested/multi-paragraph lists, complex
   blockquotes, malformed regions: rendered as labeled raw-text regions
   (`RawIsland`), editable only verbatim. Never silently normalized.
8. **MVP reparses fully after each mutation.** Incremental parsing is a
   deferred optimization (RFC-032), not an assumption baked into APIs.
9. **Saves are atomic and conflict-checked.** `atomic_write` writes a
   sibling temp file and renames. Before every write the disk fingerprint
   is compared (`bekoedit_core::conflict::detect`); external changes pause
   autosave and require an explicit user decision. Neither version is
   ever lost silently.
10. **Preview never executes document HTML.** `render_preview_html`
    converts every raw HTML event to escaped text.

## Layering

```
bekoedit-app (Dioxus shell, i18n, components)
   │  intent in, projections out
bekoedit-ui-contract (versioned serializable payloads)
   │
bekoedit-core (sessions, store, save lifecycle, conflicts)
   │                          │
bekoedit-markdown (index,     bekoedit-fs (workspace, tree,
  identity, patches, form,      safe ops, atomic write,
  islands, preview)             recovery, recents)
```

Lower layers never depend on upper layers. The four headless crates are
the workspace default-members and carry the test suite; the app crate is
a thin shell, kept deliberately logic-poor.

## Data lifecycle of one edit (RFC-000 §9)

1. UI emits a semantic command with `base_revision` and `BlockId`.
2. Core validates revision, resolves the block, verifies the fingerprint.
3. The edit resolves to a minimal `SourcePatch`; the range is validated.
4. Canonical text mutates; revision increments; dirty is set.
5. Full reparse rebuilds the index; projections rebuild from it.
6. Autosave is (re)scheduled; a recovery snapshot is written.
7. On save: conflict check → atomic write → fingerprint update →
   snapshot removal.
