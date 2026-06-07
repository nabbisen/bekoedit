# RFC-011: Text Mode with CodeMirror 6

**Project:** bekoedit  
**Status:** Proposed  
**Track:** MVP Critical  
**Milestone:** M3  
**Priority:** Critical  
**Date:** 2026-06-07  
**Related documents:** `bekoedit-requirements-definition.md`, `bekoedit-external-design.md`, `bekoedit-rfc-roadmap.md`

---

## 1. Summary

Defines raw Markdown editing using CodeMirror 6 inside the WebView, with Rust-owned canonical synchronization.

---

## 2. Motivation

- Text Mode is the safest and most essential editing mode.
- CodeMirror 6 is lightweight and modular compared to Monaco.
- Raw editing must remain first-class even after Form Mode exists.

---

## 3. Goals

- Embed CodeMirror 6 as the Text Mode editor.
- Provide Markdown syntax highlighting and basic editing commands.
- Synchronize text changes into Rust document sessions.
- Preserve raw Markdown exactly as user edits it.
- Support cursor, selection, line/column, and dirty status updates.

---

## 4. Non-Goals

- Use Monaco in MVP.
- Implement VS Code-level language services.
- Implement collaborative cursors.
- Let CodeMirror save files directly.

---

## 5. Architectural Invariants

All RFCs in this package inherit the following invariants unless explicitly amended by a later accepted RFC.

1. **Canonical source invariant:** the raw Markdown text loaded from disk is the only durable document source of truth.
2. **Projection invariant:** Text Mode, Preview Mode, Form Mode, outline data, file widgets, and parsed indexes are projections or interaction surfaces derived from canonical state.
3. **Rust ownership invariant:** the Rust core owns filesystem authority, document sessions, byte ranges, source patches, save lifecycle, recovery data, and conflict resolution.
4. **WebView boundary invariant:** JavaScript running inside the WebView may express user intent, but it must not be trusted as the authority for persistence or UTF-8 byte ranges.
5. **Source preservation invariant:** operations must preserve user-authored Markdown structure unless the user intentionally changes that structure.
6. **Safe fallback invariant:** if a Markdown region cannot be safely edited visually, it must be represented as a Raw Markdown Island rather than being silently rewritten.
7. **MVP simplicity invariant:** full reparse after document mutation is acceptable for MVP; incremental parsing is an optimization, not a prerequisite.

---

## 6. User-Facing Design

- Text Mode shows raw Markdown with syntax highlighting.
- The editor header shows file name and save state.
- Common shortcuts work: save, find in file, undo/redo, select all, toggle preview/mode.
- Line and column are shown in status bar.

---

## 7. Data Model / Contracts

```rust
struct TextChange {
    from_utf16: JsTextOffset,
    to_utf16: JsTextOffset,
    inserted_text: String,
    client_doc_version: u64,
}

struct TextModeProjection {
    document_id: DocumentId,
    revision: u64,
    text: String,
    language: EditorLanguage,
}
```

For MVP, Text Mode may send batched changes or whole-document snapshots after debounce if diff integration is not yet stable. The Rust core still owns the canonical text after accepting the update.

---

## 8. Internal Design Notes

- Define conversion between JS editor offsets and Rust string positions carefully.
- Prefer CodeMirror transaction metadata for compact changes.
- Reject stale client versions and request full refresh.
- Keep CodeMirror undo stack local to Text Mode where possible, but reset when Rust sends incompatible document replacement.

---

## 9. Data Lifecycle

1. The user action is converted into a typed command.
2. The command is validated against current application, workspace, or document state.
3. If the command may affect source text or files, the Rust core resolves authoritative paths, revisions, byte ranges, or file identities.
4. The core applies the accepted mutation or rejects the command with a user-facing error.
5. Derived projections are rebuilt or refreshed after accepted mutations.
6. The UI receives a new projection or status event and updates visible state.

For document-mutating RFCs, this lifecycle is strict. For read-only or future-evaluation RFCs, the lifecycle applies only to the parts that become implemented.

---

## 10. UI/UX Requirements

- The feature must expose clear visible state: loading, ready, dirty, saving, saved, warning, error, or conflict where applicable.
- The user must never need to understand internal parser state to recover from a normal error.
- Destructive actions must require explicit confirmation or a reversible strategy.
- The interface must preserve the focused writing experience and avoid IDE-level visual clutter.
- When a feature is unavailable for a given document region, the UI must provide a safe fallback such as Text Mode or Raw Markdown Island editing.

---

## 11. Accessibility Requirements

- All primary actions introduced by this RFC must be reachable by keyboard.
- Icon-only controls must have accessible labels.
- Focus must be visible and predictable.
- Errors and conflict states must be available to assistive technology through status text or announcements.
- Color must not be the only means of communicating state.

---

## 12. Security and Safety Considerations

- Filesystem operations must remain scoped to the active workspace unless the user explicitly chooses a path through an OS dialog.
- JavaScript-side components must not receive unrestricted filesystem authority.
- Markdown rendering must not execute untrusted script content.
- Commands crossing the WebView boundary must be validated and versioned.
- Source-preserving behavior must be tested with adversarial Markdown cases before release.

---

## 13. Testing Strategy

- Unit tests for pure core logic.
- Golden-file tests for source preservation where Markdown source is involved.
- Integration tests for command/event state transitions.
- GUI smoke tests for critical workflows where feasible.
- Regression tests for every bug that could cause source corruption or data loss.

Recommended source-preservation cases:

```text
- UTF-8 Japanese text and emoji
- LF and CRLF files
- YAML/TOML front matter
- HTML blocks
- Fenced code blocks with backticks and tildes
- Bullet lists using -, *, and +
- Ordered lists with non-1 starting numbers
- Reference links
- Blank-line-sensitive documents
- Raw Markdown Islands
```

---

## 14. Acceptance Criteria

- User can edit raw Markdown in CodeMirror.
- Text changes update Rust canonical text and dirty state.
- Mode switching away from Text Mode uses the latest accepted canonical text.
- Multibyte text edits do not corrupt UTF-8.

---

## 15. Rollout Plan

1. Implement the smallest safe vertical slice behind normal application flow.
2. Add tests before expanding supported syntax or UI states.
3. Validate behavior with real Markdown documents.
4. Document known limitations in user-facing release notes.
5. Promote the feature from draft to accepted only after source-preservation and workflow tests pass.

---

## 16. Open Questions

- Are there platform-specific constraints that require narrowing this RFC for the first beta?
- Does the feature introduce any new source-preservation risk not covered by existing golden tests?
- Should any part of this RFC be split before implementation to reduce review risk?
- What telemetry-free debugging information should be available in bug reports?
