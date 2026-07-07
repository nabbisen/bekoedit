# bekoedit RFC Index

Governance: [`done/000-rfc-lifecycle-policy.md`](done/000-rfc-lifecycle-policy.md).
Folder = state. `appendices/` holds the glossary (APPENDIX-A) and
dependency map (APPENDIX-B).

## Numbering namespaces (documented deviation)

Two documents carry the number 000:

- **`NNN`** (bare) — meta/governance (e.g. `000-rfc-lifecycle-policy.md`).
- **`RFC-NNN`** — product RFCs (e.g. `RFC-000-project-charter-and-architectural-invariants.md`).

Resolved 2026-06-07 rather than renumbering the cross-referenced corpus.

## 2026-06-07 review resolutions

1. **Split Mode** — deferred post-MVP; `EditorMode = {Text, Form, Preview}`.
   Resolution note in RFC-010.
2. **RFC-018 command set** — amended: `ReplaceListItemText`, `DeleteBlock`
   added; `ToggleTaskChecked` keyed by `item_ordinal`. `SetLinkTarget` deferred.
3. **Open Question 10** — single open document for MVP.
4. External design §36 numbering superseded by the roadmap.

---

## Implemented — v0.2.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-011 | Text Mode with CodeMirror 6 | CM6 bundle (assets/editor-bundle.js); eval-relay pattern for bidirectional bridge |
| RFC-020 | Command palette & keyboard shortcuts | Global shortcuts.js relay: Ctrl+S save, Ctrl+1/2/3 mode, Ctrl+B explorer |
| RFC-021 | Accessibility baseline & interaction contracts | role=tree/treeitem, role=tablist/tab, ARIA live regions, :focus-visible |
| RFC-022 | Settings, preferences & local configuration | AppSettings + UserSettings persisted atomically; settings screen |
| RFC-023 | Error surfaces, status bar & user feedback | Toast layer (Info/Success/Warning/Error, 4 s auto-dismiss); ARIA status/alert |

## Implemented — v0.1.0 (`done/`)

| RFC | Title |
|-----|-------|
| 000 (meta) | RFC lifecycle policy |
| RFC-000 | Project charter & architectural invariants |
| RFC-001 | Repository, toolchain & CI foundation |
| RFC-003 | Workspace model & recent workspaces |
| RFC-004 | Native file explorer & file tree index |
| RFC-006 | Document session & canonical source model |
| RFC-007 | Save, autosave, atomic write & recovery |
| RFC-008 | Dirty state, conflict detection & resolution |
| RFC-009 | Application state store & command/event model |
| RFC-013 | Markdown parser index & source range mapping |
| RFC-014 | Block identity, revision scope & projection validity |
| RFC-015 | SourcePatch engine & source-preserving mutation |
| RFC-016 | Form Mode MVP surface & safe editable blocks |
| RFC-017 | Raw Markdown Islands |
| RFC-018 | JS form adapter & semantic edit commands (amended) |
| RFC-019 | Mode switching & projection synchronization |

---

## Proposed — remaining MVP (`proposed/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-002 | Runtime architecture & WebView boundary | Contract crate ships; in-process eval relay adopted; JSON hardening ongoing |
| RFC-005 | File operations & external file watching | File ops implemented; native fs watcher (inotify/FSEvents/ReadDirectoryChanges) pending |
| RFC-010 | Main shell layout & navigation UX | Core layout ships; outline panel, split-pane resize, outline tab deferred |
| RFC-012 | Preview Mode | Sanitized rendering ships; scroll-sync deferred |
| RFC-024 | Packaging & unsigned distribution UX | Release workflow sketched in .github/workflows/release.yml |
| RFC-025 | Release CI smoke tests | CI matrix (lint/test/build) ships; smoke-test suite pending |
| RFC-026 | MVP acceptance, quality gates & beta readiness | Acceptance matrix being filled as RFCs land |

## Proposed — post-MVP (`proposed/`, deferred)

RFC-027 table editing · RFC-028 image/asset management · RFC-029 outline
operations · RFC-030 richer inline formatting · RFC-031 Lexical decision ·
RFC-032 incremental parsing performance.

## Proposed — future evaluation (`proposed/`, deferred)

RFC-033 full-text search · RFC-034 backlinks · RFC-035 export profiles ·
RFC-036 Git awareness · RFC-037 workspace templates · RFC-038 extension
policy · RFC-039 plugin system · RFC-040 sync & collaboration.

## Implemented — v0.3.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-005 | File operations & external file watching | `FsWatcher` wraps `notify` v6; restarts on workspace change |
| RFC-010 | Main shell layout & navigation UX | Split Mode, Outline panel, explorer collapse |
| RFC-012 | Preview Mode scroll sync | Proportional fractional sync in Split Mode |
| RFC-024 | Packaging & unsigned distribution UX | `docs/src/distribution.md` covering all three platforms |
| RFC-025 | Release CI smoke tests | Build-and-smoke CI job; ELOC check in lint job |
| RFC-026 | MVP acceptance, quality gates & beta readiness | `docs/src/mvp-acceptance.md` — formal v1.0 gate |

**All MVP-critical RFCs (RFC-000 through RFC-026) are now in `done/`.**

## Implemented — v0.4.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-027 | Table editing strategy | Simple (all-plaintext) GFM tables become editable cell grids; complex tables remain raw islands |
| RFC-028 | Image & asset management | Image preview cards in Form Mode with editable alt text and path |
| RFC-030 | Richer inline formatting in Form Mode | Bold/italic/code/link toolbar using UTF-16→UTF-8 offset bridge |
| RFC-033 | Full-text search | `bekoedit_fs::search_workspace` + workspace search panel with ranked results |
| RFC-035 | Export profiles | `AppState::export_html` → standalone self-contained HTML file |

## Decision reached — v0.4.0 (still `proposed/`)

| RFC | Title | Decision |
|-----|-------|---------|
| RFC-031 | Lexical integration decision | **Do not adopt Lexical.** Custom projection approach retained. See RFC for detailed rationale. |
| RFC-032 | Performance optimization & incremental parsing | Full-reparse-after-mutation confirmed adequate for current document sizes. Deferred until profiling demonstrates a need. |

## Implemented — v0.5.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-029 | Outline-based document operations | Move-section-up/down in the outline panel; engine preserves all source trivia |
| RFC-034 | Backlinks & reference discovery | `find_backlinks` scans workspace; ⬡ button opens BacklinksPanel |
| RFC-036 | Git awareness | `git status --porcelain` subprocess; M/A/D/? badges in the file explorer |
| RFC-037 | Workspace templates | `.bekoedit/templates/*.md` auto-discovered; create-from-template in AppState |

**Remaining proposed RFCs:** RFC-031 (decided: no Lexical), RFC-032 (deferred: incremental parsing), RFC-038/039/040 (future evaluation only).

## Implemented — v0.6.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-038 | Advanced Markdown extension policy | Math blocks/inline displayed as styled LaTeX source; footnotes classified as `RawIslandType::Footnote`; strikethrough via existing `ENABLE_STRIKETHROUGH` |

**RFC-032 evaluation result:** full-reparse of a 240 KB document runs in 3.57 ms (release). Incremental parsing deferred — threshold not approached.

**Remaining proposed:** RFC-031 (decided), RFC-032 (deferred), RFC-039/040 (future evaluation only).
