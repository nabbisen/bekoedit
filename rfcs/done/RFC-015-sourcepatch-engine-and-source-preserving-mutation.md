# RFC-015: SourcePatch Engine and Source-Preserving Mutation

**Project:** bekoedit  
**Status:** Implemented (v0.1.0, 2026-06-07)  
**Track:** MVP Critical  
**Milestone:** M4  
**Priority:** Critical  
**Date:** 2026-06-07  
**Related documents:** `bekoedit-requirements-definition.md`, `bekoedit-external-design.md`, `bekoedit-rfc-roadmap.md`

---

## 1. Summary

Defines the patch engine that mutates canonical Markdown text using Rust-resolved ranges and style-preserving replacement rules.

---

## 2. Motivation

- Form Mode must mutate source without rewriting unrelated Markdown.
- The patch engine is the safety-critical heart of bekoedit.

---

## 3. Goals

- Apply minimal replacements to canonical text.
- Preserve surrounding trivia and style where possible.
- Validate ranges before mutation.
- Increment document revisions and trigger reparse.
- Provide a testable API independent of the GUI.

---

## 4. Non-Goals

- Implement arbitrary semantic merge.
- Normalize entire documents.
- Guarantee perfect preservation for unsupported structures.
- Support concurrent patches against the same revision without validation.

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

- The user should see only intended changes.
- Formatting outside the edited block should remain unchanged.
- If a visual edit cannot be safely applied, the app should explain and offer raw editing.

---

## 7. Data Model / Contracts

```rust
struct SourcePatch {
    document_id: DocumentId,
    base_revision: u64,
    range: ByteRange,
    replacement: String,
    origin: PatchOrigin,
    preservation_notes: Vec<PreservationNote>,
}

enum PatchOrigin { TextMode, FormMode, RawIsland, FileRecovery }

struct PatchResult {
    new_revision: u64,
    affected_range: ByteRange,
    reparse_required: bool,
}
```

All `SourcePatch` values are constructed or approved by the Rust core.

---

## 8. Internal Design Notes

- Validate UTF-8 character boundaries before slicing.
- Ensure patch base revision matches current document revision unless an explicit rebase path exists.
- Preserve line endings by converting replacement text as needed.
- After patch, perform full reparse for MVP.
- Record diagnostics when replacement changes style intentionally.

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

- Patch application cannot slice inside a UTF-8 character.
- Patch against stale revision is rejected.
- Unrelated blocks remain byte-identical after block edit.
- Full reparse occurs after accepted patch.

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
