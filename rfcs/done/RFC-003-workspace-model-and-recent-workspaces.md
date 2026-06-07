# RFC-003: Workspace Model and Recent Workspaces

**Project:** bekoedit  
**Status:** Implemented (v0.1.0, 2026-06-07)  
**Track:** MVP Critical  
**Milestone:** M1  
**Priority:** Critical  
**Date:** 2026-06-07  
**Related documents:** `bekoedit-requirements-definition.md`, `bekoedit-external-design.md`, `bekoedit-rfc-roadmap.md`

---

## 1. Summary

Defines how bekoedit opens a local root folder, remembers recent workspaces, and scopes file navigation.

---

## 2. Motivation

- The file explorer and editor need a clear workspace root.
- Users need a fast way to reopen recent folders.
- The MVP should avoid complexity from multi-root workspace semantics.

---

## 3. Goals

- Support one active workspace root for MVP.
- Remember recent workspace folders locally.
- Validate workspace availability and permissions.
- Provide an empty-state flow when no workspace is open.

---

## 4. Non-Goals

- Support multi-root workspaces in MVP.
- Implement remote workspaces.
- Implement project-specific plugin configuration.
- Index every file type.

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

- Startup screen shows recent workspaces and an Open Folder action.
- If the last workspace is available, the app may offer to reopen it.
- If a workspace path no longer exists, show a clear recovery message and remove/keep option.

---

## 7. Data Model / Contracts

```rust
struct Workspace {
    id: WorkspaceId,
    root_path: PathBuf,
    display_name: String,
    opened_at: DateTime,
    file_tree_revision: u64,
}

struct RecentWorkspaceEntry {
    root_path: PathBuf,
    display_name: String,
    last_opened_at: DateTime,
    last_selected_file: Option<PathBuf>,
}
```

Recent workspace data is local app configuration, not written into the user's Markdown folder unless a later RFC introduces workspace-local metadata.

---

## 8. Internal Design Notes

- Use canonicalized paths where safe, but preserve user-friendly display paths.
- Do not follow symlink loops while indexing.
- Keep recent workspace storage separate from document recovery storage.
- Validate workspace access before starting file watcher.

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

- User can open one local folder as a workspace.
- Recent workspace list survives app restart.
- Unavailable workspace paths are handled without crash.
- Opening a new workspace closes or prompts for dirty documents from the current workspace.

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
