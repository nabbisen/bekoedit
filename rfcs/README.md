# bekoedit RFC Index

Governance: [`done/000-rfc-lifecycle-policy.md`](done/000-rfc-lifecycle-policy.md)
— a **verbatim mirror** of a policy shared across projects. Do not edit it here;
anything project-specific belongs on this page instead.

**Folder = state**, and the folder wins if a Status field disagrees with it.

This project uses the policy's **5-folder variant**, adopted 2026-08-24:

| Folder | State |
|---|---|
| [`proposed/`](proposed/) | Open for review; implementer should not start |
| [`accepted/`](accepted/) | Owner signed off; implementer may start; not shipped |
| [`done/`](done/) | Implemented |
| [`archive/`](archive/) | Withdrawn or superseded (currently empty) |

`accepted/` exists here because the condition the policy names is met: design
and implementation are genuinely separate roles. The architect writes and
revises RFCs and issues handoffs; a separate dev team implements them; and "the
owner approved this design" is a distinct, dated event from "the implementer
finished." Under the 4-folder layout that distinction had no home, so
approved-but-unbuilt RFCs sat in `proposed/` carrying an in-file qualifier that
contradicted their folder.

The policy warns that `accepted/` is only worth having if it does not sit empty.
It held three RFCs on the day the variant was adopted.

`handoffs/` holds companion execution documents and is deliberately **not**
split by state — a handoff inherits its status from its RFC's folder.
`appendices/` holds the glossary (APPENDIX-A) and dependency map (APPENDIX-B).

## Numbering namespaces (documented deviation)

Two documents carry the number 000:

- **`NNN`** (bare) — meta/governance (e.g. `000-rfc-lifecycle-policy.md`).
- **`RFC-NNN`** — product RFCs (e.g. `RFC-000-project-charter-and-architectural-invariants.md`).

Resolved 2026-06-07 rather than renumbering the cross-referenced corpus.

## 2026-06-07 review resolutions

1. **Split Mode** — initially deferred from the earliest MVP cut, then
   implemented in v0.3.0. Current `EditorMode` includes
   `{Text, Form, Preview, Split}`.
2. **RFC-018 command set** — amended: `ReplaceListItemText`, `DeleteBlock`
   added; `ToggleTaskChecked` keyed by `item_ordinal`. `SetLinkTarget` deferred.
3. **Open Question 10** — single open document for MVP.
4. External design §36 numbering superseded by the roadmap.

---

## Implemented — v0.2.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-011 | [Text Mode with CodeMirror 6](done/RFC-011-text-mode-with-codemirror-6.md) | CM6 bundle (assets/editor-bundle.js); eval-relay pattern for bidirectional bridge |
| RFC-020 | [Command palette & keyboard shortcuts](done/RFC-020-command-palette-and-keyboard-shortcut-system.md) | Global shortcuts.js relay: Ctrl+S save, Ctrl+1/2/3 mode, Ctrl+B explorer |
| RFC-021 | [Accessibility baseline & interaction contracts](done/RFC-021-accessibility-baseline-and-interaction-contracts.md) | role=tree/treeitem, role=tablist/tab, ARIA live regions, :focus-visible |
| RFC-022 | [Settings, preferences & local configuration](done/RFC-022-settings-preferences-and-local-configuration.md) | AppSettings + UserSettings persisted atomically; settings screen |
| RFC-023 | [Error surfaces, status bar & user feedback](done/RFC-023-error-surfaces-status-bar-and-user-feedback.md) | Toast layer (Info/Success/Warning/Error, 4 s auto-dismiss); ARIA status/alert |

## Implemented — v0.1.0 (`done/`)

| RFC | Title |
|-----|-------|
| 000 (meta) | RFC lifecycle policy |
| RFC-000 | [Project charter & architectural invariants](done/RFC-000-project-charter-and-architectural-invariants.md) |
| RFC-001 | [Repository, toolchain & CI foundation](done/RFC-001-repository-toolchain-and-ci-foundation.md) |
| RFC-003 | [Workspace model & recent workspaces](done/RFC-003-workspace-model-and-recent-workspaces.md) |
| RFC-004 | [Native file explorer & file tree index](done/RFC-004-native-file-explorer-and-file-tree-index.md) |
| RFC-006 | [Document session & canonical source model](done/RFC-006-document-session-and-canonical-source-model.md) |
| RFC-007 | [Save, autosave, atomic write & recovery](done/RFC-007-save-autosave-atomic-write-and-recovery.md) |
| RFC-008 | [Dirty state, conflict detection & resolution](done/RFC-008-dirty-state-conflict-detection-and-external-modification-resolution.md) |
| RFC-009 | [Application state store & command/event model](done/RFC-009-application-state-store-and-command-event-model.md) |
| RFC-013 | [Markdown parser index & source range mapping](done/RFC-013-markdown-parser-index-and-source-range-mapping.md) |
| RFC-014 | [Block identity, revision scope & projection validity](done/RFC-014-block-identity-revision-scope-and-projection-validity.md) |
| RFC-015 | [SourcePatch engine & source-preserving mutation](done/RFC-015-sourcepatch-engine-and-source-preserving-mutation.md) |
| RFC-016 | [Form Mode MVP surface & safe editable blocks](done/RFC-016-form-mode-mvp-surface-and-safe-editable-blocks.md) |
| RFC-017 | [Raw Markdown Islands](done/RFC-017-raw-markdown-islands.md) |
| RFC-018 | [JS form adapter & semantic edit commands (amended)](done/RFC-018-js-form-adapter-and-semantic-edit-commands.md) |
| RFC-019 | [Mode switching & projection synchronization](done/RFC-019-mode-switching-and-projection-synchronization.md) |

---

## Accepted — approved, not yet shipped (`accepted/`)

The owner has signed off on the design and an implementer may start. These move
to `done/` when the work ships.

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-043 | [Reopen last workspace on launch](accepted/RFC-043-reopen-last-workspace-on-launch.md) | Accepted 2026-08-04; not yet built. Handoff ready: [`handoffs/043-reopen-last-workspace-on-launch/`](handoffs/043-reopen-last-workspace-on-launch/implementation-handoff.md) |
| RFC-044 | [Shell behaviour regression coverage](accepted/RFC-044-shell-behaviour-regression-coverage.md) | Accepted 2026-08-24, all §14 questions resolved — reproducible coverage for RFC-042's keyboard contracts. Its slices are blocked on RFC-043 shipping (required dependency); the §7 JavaScript relocation is a prerequisite task outside the RFC and is dispatchable now |
| RFC-045 | [Release artifact portability and completeness](accepted/RFC-045-release-artifact-portability-and-completeness.md) | Accepted 2026-08-17. Slices 1–2 shipped to `main`: the platform scripts now ship in every archive, and a cross-distribution `ldd` check gates both pull requests and the publish job. Slice 3 (Linux portability) is open — see its §10 Q1. Handoffs: [`handoffs/045-release-artifact-portability-and-completeness/`](handoffs/045-release-artifact-portability-and-completeness/) |

## Open — under review or deferred (`proposed/`)

`proposed/` holds RFCs nobody has approved for implementation: under active
review, or deliberately parked. A deferral is not an abandonment — each file
carries a dated in-file qualifier explaining why it has not been pursued. An
RFC whose question has been *answered* does not belong here; it moves to
`accepted/` (approved), `done/` (shipped) or `archive/` (withdrawn or
superseded).

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-032 | [Performance optimization and incremental parsing](proposed/RFC-032-performance-optimization-and-incremental-parsing.md) | Deferred until profiling shows full reparse is insufficient |
| RFC-039 | [Plugin system evaluation](proposed/RFC-039-plugin-system-evaluation.md) | Future evaluation only |
| RFC-040 | [Sync and collaboration evaluation](proposed/RFC-040-sync-and-collaboration-evaluation.md) | Future evaluation only |

## Implemented — v0.3.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-005 | [File operations & external file watching](done/RFC-005-file-operations-and-external-file-watching.md) | `FsWatcher` wraps `notify` v6; restarts on workspace change |
| RFC-010 | [Main shell layout & navigation UX](done/RFC-010-main-shell-layout-and-navigation-ux.md) | Split Mode, Outline panel, explorer collapse |
| RFC-012 | [Preview Mode scroll sync](done/RFC-012-preview-mode-and-rendered-markdown-display.md) | Proportional fractional sync in Split Mode |
| RFC-024 | [Packaging & unsigned distribution UX](done/RFC-024-packaging-and-unsigned-distribution-ux.md) | `docs/src/distribution.md` covering all three platforms |
| RFC-025 | [Release CI smoke tests](done/RFC-025-release-ci-smoke-tests-and-build-verification.md) | Build-and-smoke CI job; ELOC check in lint job |
| RFC-026 | [MVP acceptance, quality gates & beta readiness](done/RFC-026-mvp-acceptance-quality-gates-and-beta-readiness.md) | `docs/src/mvp-acceptance.md` — formal v1.0 gate |

**All MVP-critical RFCs (RFC-000 through RFC-026) are now in `done/`.**

## Implemented — v0.4.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-027 | [Table editing strategy](done/RFC-027-table-editing-strategy.md) | Simple (all-plaintext) GFM tables become editable cell grids; complex tables remain raw islands |
| RFC-028 | [Image & asset management](done/RFC-028-image-and-asset-management.md) | Image preview cards in Form Mode with editable alt text and path |
| RFC-030 | [Richer inline formatting in Form Mode](done/RFC-030-richer-inline-formatting-in-form-mode.md) | Bold/italic/code/link toolbar using UTF-16→UTF-8 offset bridge |
| RFC-033 | [Full-text search](done/RFC-033-full-text-search.md) | `bekoedit_fs::search_workspace` + workspace search panel with ranked results |
| RFC-035 | [Export profiles](done/RFC-035-export-profiles.md) | `AppState::export_html` → standalone self-contained HTML file |

## Decision reached — v0.4.0

| RFC | Title | Decision |
|-----|-------|---------|
| RFC-031 | [Lexical integration decision](done/RFC-031-lexical-integration-decision.md) | **Do not adopt Lexical.** Custom projection approach retained. See RFC for detailed rationale. Moved to `done/` on 2026-07-31; the decision shipped in v0.4.0 and is standing guidance, not open review. |
| RFC-032 | [Performance optimization & incremental parsing](proposed/RFC-032-performance-optimization-and-incremental-parsing.md) | Full-reparse-after-mutation confirmed adequate for current document sizes. Deferred until profiling demonstrates a need. |

## Implemented — v0.5.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-029 | [Outline-based document operations](done/RFC-029-outline-based-document-operations.md) | Move-section-up/down in the outline panel; engine preserves all source trivia |
| RFC-034 | [Backlinks & reference discovery](done/RFC-034-backlinks-and-reference-discovery.md) | `find_backlinks` scans workspace; ⬡ button opens BacklinksPanel |
| RFC-036 | [Git awareness](done/RFC-036-git-awareness.md) | `git status --porcelain` subprocess; M/A/D/? badges in the file explorer |
| RFC-037 | [Workspace templates](done/RFC-037-workspace-templates.md) | `.bekoedit/templates/*.md` auto-discovered; create-from-template in AppState |

**Remaining proposed RFCs:** RFC-031 (decided: no Lexical), RFC-032
(deferred: incremental parsing), RFC-039/040 (future evaluation only).

## Implemented — v0.6.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-038 | [Advanced Markdown extension policy](done/RFC-038-advanced-markdown-extension-policy.md) | Math blocks/inline displayed as styled LaTeX source; footnotes classified as `RawIslandType::Footnote`; strikethrough via existing `ENABLE_STRIKETHROUGH` |

**RFC-032 evaluation result:** full-reparse of a 240 KB document runs in 3.57 ms (release). Incremental parsing deferred — threshold not approached.

**Remaining proposed:** RFC-031 (decided), RFC-032 (deferred), RFC-039/040 (future evaluation only).

## Implemented — v0.9.0–v0.10.1 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-002 | [Runtime architecture and WebView boundary](done/RFC-002-runtime-architecture-and-webview-boundary.md) | Typed versioned payloads, eval relay with auto-restart (v0.9.0), rfd native dialogs replacing text-path input (v0.10.0) |

**All MVP RFCs now implemented.** Remaining proposed: RFC-031 (decided), RFC-032 (deferred), RFC-039/040 (future evaluation only).

## Implemented — v0.14.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-042 | [Shell interaction, focus & accessibility conformance](done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) | One arbitrated focus authority shared by the shell and the RFC-041 controller; WAI-ARIA keyboard contracts for the workspace tree, overflow menus, and mode tabs; accessibility metadata for the conflict banner, Recovery, Settings, and Form Mode blocks. Slices 1–4 in 0.14.0; slice 5 merged after the tag and ships next release; slice 6 withdrawn. See its §13.1. |

Companion handoffs: [`handoffs/042-shell-interaction-focus-and-accessibility-conformance/`](handoffs/042-shell-interaction-focus-and-accessibility-conformance/)
— five slice documents, status inherited from this RFC.

## Implemented — v0.13.0 (`done/`)

| RFC | Title | Notes |
|-----|-------|-------|
| RFC-041 | [Source editor lifecycle and synchronization controller](done/RFC-041-source-editor-lifecycle-and-synchronization-controller.md) | Rust-owned protocol-v2 lifecycle, correlated source barriers, explicit mount/refresh/teardown, and validated Text/Split focus |
