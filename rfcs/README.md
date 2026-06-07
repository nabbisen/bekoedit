# bekoedit RFC Index

Governance: [`done/000-rfc-lifecycle-policy.md`](done/000-rfc-lifecycle-policy.md).
Folder = state: `proposed/` (accepted direction, not fully implemented),
`done/` (implemented; status names the shipping version), `archive/`
(superseded/withdrawn). `appendices/` holds the shared glossary
(APPENDIX-A) and dependency map (APPENDIX-B).

## Numbering namespaces (documented deviation)

Two documents carry the number 000. They live in different namespaces:

- **`NNN`** (bare) — meta/governance documents about the RFC process
  itself, e.g. `000-rfc-lifecycle-policy.md`.
- **`RFC-NNN`** — product RFCs, e.g.
  `RFC-000-project-charter-and-architectural-invariants.md`.

This was resolved during the 2026-06-07 review rather than renumbering 41
cross-referenced documents.

## Review resolutions (2026-06-07)

1. **Split Mode** (external design §16 vs RFC-010): deferred post-MVP;
   `EditorMode` stays `{Text, Form, Preview}`. Resolution note added to
   RFC-010.
2. **Form command set** (RFC-018 vs external design §23.11): RFC-018
   amended to include `ReplaceListItemText{item_ordinal,text}` and
   `DeleteBlock`; `ToggleTaskChecked` keyed by `item_ordinal`.
   `SetLinkTarget` remains specified but ships post-v0.1.0.
3. **Requirements Open Question 10**: MVP keeps a single open document.
4. External design §36's provisional RFC numbering is superseded by the
   roadmap; the roadmap and this index are authoritative.

## Implemented in v0.1.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| 000 (meta) | RFC lifecycle policy | adopted as written |
| RFC-000 | Project charter & architectural invariants | invariants restated normatively in `ARCHITECTURE.md` |
| RFC-001 | Repository, toolchain & CI foundation | workspace, edition 2024, fmt/clippy/test CI, 3-OS matrix |
| RFC-003 | Workspace model & recent workspaces | |
| RFC-004 | Native file explorer & file tree index | watching is polling-based via RFC-008 checks, see RFC-005 note |
| RFC-006 | Document session & canonical source model | |
| RFC-007 | Save, autosave, atomic write & recovery | recovery restore UI minimal (snapshots listed via store API) |
| RFC-008 | Dirty state, conflict detection & resolution | |
| RFC-009 | Application state store & command/event model | |
| RFC-013 | Markdown parser index & source range mapping | |
| RFC-014 | Block identity, revision scope & projection validity | |
| RFC-015 | SourcePatch engine & source-preserving mutation | |
| RFC-016 | Form Mode MVP surface & safe editable blocks | |
| RFC-017 | Raw Markdown Islands | |
| RFC-018 | JS form adapter & semantic edit commands | as amended; `SetLinkTarget` deferred |
| RFC-019 | Mode switching & projection synchronization | |

## Proposed — MVP polish (`proposed/`)

| RFC | Title | v0.1.0 status |
|-----|-------|---------------|
| RFC-002 | Runtime architecture & WebView boundary | partially implemented: contract crate ships; in-process Dioxus shell uses it as the typed boundary; JSON bridge hardening continues |
| RFC-005 | File operations & external file watching | ops implemented; native watcher pending (polling detection in place) |
| RFC-010 | Main shell layout & navigation UX | core layout shipped; outline panel & polish pending |
| RFC-011 | Text Mode with CodeMirror 6 | snapshot contract shipped behind an interim surface; CM6 adapter pending |
| RFC-012 | Preview Mode | sanitized rendering shipped; scroll-sync & theming pending |
| RFC-020 | Command palette & keyboard shortcuts | |
| RFC-021 | Accessibility baseline | |
| RFC-022 | Settings & local configuration | recents persistence shipped; settings UI pending |
| RFC-023 | Error surfaces & status bar | status bar shipped; toast/error panel pending |
| RFC-024 | Packaging & unsigned distribution UX | release workflow sketched |
| RFC-025 | Release CI smoke tests | |
| RFC-026 | MVP acceptance & beta readiness | |

## Proposed — post-MVP (`proposed/`, deferred)

RFC-027 table editing · RFC-028 image/asset management · RFC-029 outline
operations · RFC-030 richer inline formatting · RFC-031 Lexical decision ·
RFC-032 incremental parsing performance.

## Proposed — future evaluation (`proposed/`, deferred)

RFC-033 full-text search · RFC-034 backlinks · RFC-035 export profiles ·
RFC-036 Git awareness · RFC-037 workspace templates · RFC-038 extension
policy · RFC-039 plugin system · RFC-040 sync & collaboration.
