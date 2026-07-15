# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.13.0] - 2026-07-15

### Added
- A Rust-owned source-editor lifecycle controller now coordinates bundle
  readiness, editor identity, correlated snapshots, hold/resume, refresh, and
  teardown across the Dioxus/WebView boundary.
- Bridge protocol v2 types, lifecycle reducer tests, JavaScript adapter tests,
  and focus-transition tracing cover Text and Split editor mounts.
- Blocking Linux CI now runs the Rust app and JavaScript adapter suites plus an
  actual Dioxus/WebView New-to-Preview lifecycle regression under Xvfb.

### Changed
- New documents open directly in Text Mode and focus CodeMirror after its
  validated ready handshake.
- Workspace search uses a focused Explorer overlay, clears stale results while
  the query changes, and keeps unsupported files visibly disabled.
- Editor modes, menus, recovery notices, split controls, and toolbar icons now
  expose clearer active, dismissal, and availability states.

### Fixed
- First-run and remount races no longer leave the editor unavailable, emit
  repeated source-operation timeout errors, or stall Preview transitions.
- Recovery no longer prompts on every edit of a new document, and restored
  notices can be dismissed or expire automatically.
- Overflow menus remain within the window, close when focus moves outside,
  and no longer route unrelated actions to Settings.
- Split Mode can be closed, search no longer shows premature or stale
  no-match results, and truncated Explorer labels retain their full tooltip.

[0.13.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.13.0

## [0.12.0] - 2026-07-11

### Added
- Release evidence template and sign-off workflow covering local gates,
  remote CI, release artifacts, checksums, and manual walkthrough evidence.
- Security audit and release checksum gates for release readiness evidence.

### Changed
- CI smoke testing is now blocking, and release archives unpack directly into
  the destination root.
- Release tags now use bare SemVer, for example `0.12.0`, matching Rust
  release convention.
- Startup recovery now presents pending dirty snapshots through the app shell
  with explicit recover and discard actions.
- Readiness docs now separate observed local evidence, owner-provided remote
  CI evidence, and release-artifact evidence still required after tagging.

### Fixed
- Pending conflict guards now block section moves and history restore, closing
  stale-baseline source-safety gaps.
- Recovery restore APIs preserve dirty/recovery lifecycle state and reject
  ambiguous restore targets.
- The CI ELOC check no longer fails when no files exceed the limit.
- Cross-platform recents persistence tests now compare canonical workspace
  roots, fixing macOS and Windows CI failures.

[0.12.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.12.0

## [0.11.5] - 2026-07-07

- Crate attributes.

[0.11.5]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.5

## [0.11.4] - 2026-07-07

- Codebase housekeeping.
- `ARCHITECTURE.md` moved to `docs/src/architectural-invariants.md`.
- `CONTRIBUTING.md` moved to `.github/CONTRIBUTING.md`.

[0.11.4]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.4

## [0.11.3] - 2026-06-09

### Fixed
- **"New File" on the start screen did nothing.** `new_untitled()` creates a
  session without opening a workspace, but the navigation guard in `app.rs`
  was `workspace.is_some()`, so the editor never appeared. Guard widened to
  `workspace.is_some() || session.is_some()`.

- **Clicking a file showed "Dragging 1 item(s)" and never opened the
  document.**  Root cause: `dioxus-swdir-tree`'s `TreeRow.on_mouseup` only
  dispatches `DragMsg::Released` when `is_drag_active` is `true`, but
  `is_drag_active` is captured at last-render time. In Dioxus Desktop a fast
  click (mousedown → mouseup before the next repaint) leaves `is_drag_active`
  stale at `false`, so `Released` never fires, the drag state is never
  cleared, the "Dragging N item(s)" overlay persists, and `DragOutcome::Clicked`
  — the only path to `open_document` — is never reached.

  Fix: replaced `DirectoryTreeView` with a custom `TreeRowItem` component
  that renders each visible row with a plain `onclick` handler. The
  `DirectoryTree` state machine and `use_scan_driver` from the library are
  still used for lazy directory loading and the prefetch-skip list. Only the
  drag-and-drop rendering layer is bypassed — bekoedit has no use for
  drag-and-drop in the file tree.

[0.11.3]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.3

## [0.11.2] - 2026-06-09

### Testing review

Reviewed the project against the Dioxus 0.7 testing guide (component SSR
testing, hook testing, Playwright E2E). Decision: **do not adopt any of
them** — each would add complexity disproportionate to its value for a
Desktop WebView app whose correctness lives in pure-logic crates:

- Component SSR testing: our components are thin context-driven plumbing
  that delegate to already-tested core logic; SSR-diffing HTML strings is
  brittle and needs new dependencies.
- Hook testing: bekoedit defines no custom hooks (the one in use,
  `use_scan_driver`, belongs to dioxus-swdir-tree).
- Playwright E2E: targets `dx serve` (web); bekoedit is Desktop. The
  `--headless-smoke` binary already covers the integration path.

Testing stays concentrated where correctness matters: the markdown, fs,
and core crates (no new dev-dependencies; `tempfile` remains the only one).

### Added
- `tests/untitled_tests.rs` (7 tests) covering the v0.10/v0.11 AppState
  lifecycle methods: `new_untitled()`, `save_as()`, `close_workspace()`.
  Plain Rust unit tests, no UI harness.
- Test count: 131 → 133.

### Fixed
- **"New File → Save As" silently wrote nothing for an empty document.**
  `DocumentSession::new_untitled()` created the session with `dirty: false`
  while `AppState::new_untitled()` set `save_state = Dirty`. On Save As,
  `save_now()` hit its `if !session.dirty { return Ok(()) }` early-return
  and skipped the write. The session is now created with `dirty: true`
  (a new in-memory document genuinely has unsaved state), so Save As always
  persists it. Caught by `save_as_writes_to_disk_and_clears_untitled`.

[0.11.2]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.2

## [0.11.1] - 2026-06-09

### Design: Less is more

Applied the "Less is more" principle throughout the UI. First-time users
now see substantially fewer controls at a glance; advanced features are
accessible but not in the way.

**Visible control count before → after:**

| Zone | Before | After |
|------|--------|-------|
| AppBar | 4 (logo, File▾, Language, ⚙) | 2 (logo, ⋯) |
| EditorHeader | 15 (6 panel toggles, 4 mode tabs, undo, redo, export, save, save-as) | 5 (explorer, filename, Text/Preview/Form tabs, Save) |
| Explorer | 4 (+ New, ↻ Refresh, template dropdown, tree) | 2 (+ New, tree) |
| StatusBar | 5 (save state, word count, line ending, islands, diagnostics) | 1 (save state — detail on hover) |
| **Total** | **28** | **10** |

**Tier model:**

- **Tier 1 (always visible):** filename · save state · Text / Preview / Form mode tabs · Save
- **Tier 2 (on demand):** Form Mode is in the primary mode bar but visually subdued
- **Tier 3 (power, behind "•••"):** Split · Outline · Search · Backlinks · History · Export HTML

**Specific changes:**

- `AppBar`: two items only — "bekoedit" (home) + "⋯" overflow menu. The File
  menu, Language toggle, and Settings gear moved inside ⋯.
- `EditorHeader`: undo/redo toolbar buttons removed (keyboard shortcuts suffice);
  all panel toggles moved into the "•••" dropdown; only Text, Preview, and Form
  tabs remain in the primary bar.
- `StatusBar`: one label only. Word count, line ending, island count, and
  diagnostic count moved to a tooltip (`title` attribute) on the save-state span.
- Explorer: refresh button removed (swdir-tree auto-refreshes; manual refresh
  still available via "•••" or tree events).

[0.11.1]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.1

## [0.11.0] - 2026-06-07

### Fixed
- **Window menu did not work** — `dioxus::desktop::Config::with_menu(None)`
  suppresses the OS-provided native menu bar (macOS "Window" menu, etc.).
  bekoedit now manages all menus through its own UI.
- **`cargo run` fails from workspace root** — `.cargo/config.toml` now defines
  a `run-app` alias: `cargo run-app` launches bekoedit from any directory
  in the workspace. The underlying reason (`cargo run` requires `-p` in a
  virtual workspace) is documented in CONTRIBUTING.md.
- **Editor never started when a file was selected** — two root causes:
  1. `DirectoryTree::new()` starts with the root node uncollapsed but
     *not loaded* (`is_expanded: false, is_loaded: false`). A `use_effect`
     hook now triggers `on_toggled(root)` immediately on mount so the root
     directory's children appear without user interaction.
  2. The `Selected` event handler silently dropped errors from
     `open_document`. Errors are now surfaced as toasts.
  Additionally: only `.md`/`.markdown` files were opened; all files now
  open (non-Markdown files open in Text Mode).
- **Duplicate Settings / Language controls in the body** — the Settings
  gear and Language toggle that were duplicated inside `EditorHeader` are
  removed. Both live exclusively in the new `AppBar`.

### Added
- **Persistent `AppBar`** (always visible, all screens):
  - *bekoedit* logo — click to call `close_workspace()` and return to the
    start screen.
  - *File* dropdown menu — "Open Folder…", "New File", "Close Workspace".
  - Language toggle — EN ↔ JA.
  - Settings gear.
- **Undo ↩ / Redo ↪ buttons** in `EditorHeader` — call
  `window.__bk.undo()` / `window.__bk.redo()` via `document::eval`.
  `editor.js` exports `undo()` and `redo()` on `window.__bk`; the CM6
  bundle is rebuilt.
- **`AppState::close_workspace()`** — clears workspace, tree, and session;
  resets save state and autosave; returns the UI to the start screen.

[0.11.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.11.0

## [0.10.1] - 2026-06-07

### Changed
- **ELOC compliance**: `bekoedit-fs/src/tests.rs` (346 ELOC) split into
  `tests/file_system_tests.rs` (116 ELOC), `tests/persistence_tests.rs`
  (222 ELOC), and `tests/adv_tests.rs` (145 ELOC). No file now exceeds
  300 ELOC; the 500 hard limit has no violations.
- **RFC-002 marked Implemented**: the runtime architecture and WebView
  boundary RFC is complete. All its requirements are satisfied: typed
  `ui-contract` crate with versioned payloads, eval relay with auto-restart,
  `bridge::relay_js` for consistent setup, and `rfd` native dialogs
  replacing the manual text-path entry at startup.
- **Test count**: 131 tests (2 new fs persistence tests added in the split).
- RFC-002 moved from `rfcs/proposed/` to `rfcs/done/`.

[0.10.1]: https://github.com/nabbisen/bekoedit/releases/tag/0.10.1

## [0.10.0] - 2026-06-07

### Fixed
- **Start screen: native folder picker** — "Open Folder" now opens the OS
  native folder selection dialog via `rfd::AsyncFileDialog::pick_folder()`.
  Users no longer need to type a directory path manually.
- **Start screen: "New File" button** — creates a blank in-memory document
  without requiring a workspace. The document is labelled "Untitled" in the
  editor header. `save_now()` returns `StoreError::Untitled` so the UI knows
  to show a "Save As" dialog (`rfd::AsyncFileDialog::save_file()`).

### Changed (improvement)
- **Explorer replaced with `dioxus-swdir-tree` v0.7** — the hand-rolled
  recursive file tree scanner is replaced by `DirectoryTreeView`, providing:
  - **Lazy loading**: one directory level per user expand gesture; a million-
    file home directory costs only what you actually open (vs. upfront full scan)
  - **Built-in keyboard navigation**: arrow keys, Enter, F2, Delete
  - **Generation-tagged scans**: stale async scan results cannot corrupt the tree
  - **`DEFAULT_PREFETCH_SKIP`**: `.git`, `node_modules`, `target`, `build`,
    `dist`, `__pycache__`, `.venv` already ignored — no custom filter needed
  - File-create toolbar and template selector retained above the tree

### Added
- `rfd = "0.17"` dependency for native OS file/folder dialogs
- `dioxus-swdir-tree = "0.7"` dependency for the directory tree widget
- `DocumentSession::new_untitled()` — creates an in-memory session with
  `is_untitled: true`; saves are blocked until the user picks a path
- `AppState::new_untitled()` and `AppState::save_as(path)` — entry point
  for the New/Save-As workflow
- `StoreError::Untitled` variant

[0.10.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.10.0

## [0.9.1] - 2026-06-07

### Dependencies

**Updated:**

| Crate | From | To | Type |
|-------|------|----|------|
| `notify` | 6.1.1 | **8.2.0** | Direct dep — major version upgrade; our API surface (`recommended_watcher`, `EventKind`, `RecursiveMode`) is unchanged |

**Cannot update (pinned by upstream crates):**

| Crate | Lock | Available | Pinned by |
|-------|------|-----------|-----------|
| `generic-array` | 0.14.7 | 0.14.9 | `block-buffer` / `crypto-common` (sha2 dep chain) |
| `toml` | 0.8.2 | 0.8.23 | `system-deps 6.2.2` (gtk build-time dep) |
| `toml_datetime` | 0.6.3 | 0.6.11 | same |
| `toml_edit` | 0.20.2 | 0.20.7 | same |
| `webkit2gtk` | 2.0.1 | 2.0.2 | `wry 0.53.5` (Dioxus WebView layer) |
| `webkit2gtk-sys` | 2.0.1 | 2.0.2 | same |

The `webkit2gtk` and `wry` versions will update when Dioxus releases a new
version that accepts `webkit2gtk 2.0.2`. The crypto and build-tool pins
will resolve when those upstream crates release compatible versions.

[0.9.1]: https://github.com/nabbisen/bekoedit/releases/tag/0.9.1

## [0.9.0] - 2026-06-07

### Added
- **Recovery screen** (RFC-007 UI gap closed): on startup, if
  `RecoveryStore::list()` returns pending snapshots, the app shows a
  recovery screen before the start screen. Each snapshot shows its file
  path and revision with "Restore" (loads into a dirty edit session) and
  "Discard" (removes the snapshot) buttons. "Dismiss all" clears all
  snapshots without opening any file.
- **Large-file warning** in the Explorer file-open handler: files larger
  than 1.5 MB trigger an info toast before the document loads, warning
  the user that performance may be affected.
- **Relay auto-restart** (RFC-002 hardening): the shortcut-relay
  `use_coroutine` now wraps the inner `while let Ok` loop in a `for`
  restart loop (up to `MAX_RELAY_RESTARTS = 10` attempts), with a 500 ms
  backoff. If the relay eval disconnects mid-session, it automatically
  reconnects rather than silently stopping.
- `AppState::file_size_bytes(relative)` — public query for the size of a
  workspace-relative file without loading it.

### Changed
- `AppState::recovery` field changed from `pub(crate)` to `pub` so
  `RecoveryScreen` can call `recovery.list()` and `recovery.remove()`.
- Recovery screen CSS added to `style.css`.
- CHANGELOG, ROADMAP, and acceptance checklist updated.

[0.9.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.9.0

## [0.8.0] - 2026-06-07

### Added
- **IME composition guard in CodeMirror 6** (RFC-011): `editor.js` now
  tracks `compositionstart`/`compositionend` DOM events via
  `EditorView.domEventHandlers`. The debounce timer is cancelled on
  `compositionstart` and an immediate flush fires on `compositionend`.
  Partial kana/hanzi strings are never sent to Rust during active
  composition. CM6 bundle rebuilt.
- **User-facing error messages** for all `StoreError` and `FileOpError`
  variants: `error_keys.rs` maps each variant to an i18n key, including
  OS-specific classification of permission-denied vs disk-full save
  failures. Keys added in both EN and JA tables.
- **Settings persistence helpers** (`bekoedit_fs::save_user_settings`,
  `load_user_settings`): tested by `settings_persist_across_app_state_restart`.
- **Recent-workspaces persistence test**: verifies `RecentWorkspaces`
  serialises to disk and is readable by a fresh instance.
- **Large workspace stress test**: 500 Markdown files in 20 subdirectories
  scanned in < 2 s; ignore-directories test confirms `node_modules` and
  `target` are excluded.
- **Platform scripts**: `scripts/run-macos.sh` (removes quarantine attribute)
  and `scripts/run-windows.ps1` (unblocks Zone.Identifier ADS) to help
  users past unsigned-binary warnings on first launch.
- **Production README**: GitHub badge, feature matrix, Quick Start with
  platform-specific instructions, feature summary, design notes, and
  link index. Replaces the earlier stub.
- **Scroll-fraction reporter** in `editor.js`: CM6 scroll events send
  `{type:"scrollFraction", fraction}` to the relay, enabling precise
  Split Mode preview synchronisation.

### Changed
- `bekoedit-core/src/tests.rs` split into `tests/session_tests.rs`,
  `tests/delete_tests.rs`, `tests/persistence_tests.rs`.
- `bekoedit-fs/src/tests.rs` cleaned and deduplicated (346 ELOC — under
  the 500 hard limit; further splitting noted for a future pass).
- Acceptance checklist: IME item upgraded from ⚠️ to ✅ with code
  evidence; manual walkthrough note retained.

[0.8.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.8.0

## [0.7.2] - 2026-06-07

### Added
- **Adversarial golden document test suite** (8 tests): one Markdown file
  containing every tricky pattern from the MVP acceptance checklist —
  CRLF throughout, Japanese + emoji in headings, tilde fences, non-1
  ordered lists, reference links, front matter, raw HTML, and GFM tables.
  Each test edits exactly one block and asserts that every byte outside
  the patched range is unchanged.
- **i18n coverage test** (`i18n_all_keys_have_both_languages`): iterates
  every known UI key and asserts EN and JA translations both exist.
  Missing translations are reported as test failures, not runtime
  fallback strings.
- **Dirty-document delete blocked**: `AppState::delete_path` now returns
  `StoreError::DocumentDirty` (new variant) when the target is the open
  dirty document. Tests: `delete_dirty_document_returns_document_dirty`
  and `delete_clean_document_succeeds`.
- **Full detailed acceptance checklist** restored in
  `docs/src/mvp-acceptance.md`: every item has a status (✅ / ⚠️) and
  a precise evidence pointer. One item (IME composition) remains ⚠️
  pending manual verification on the release candidate.

### Changed
- `StoreError` gains a `DocumentDirty` variant (distinct from
  `ConflictPending`) for clearer UI error messaging.

[0.7.2]: https://github.com/nabbisen/bekoedit/releases/tag/0.7.2

## [0.7.1] - 2026-06-07

### Changed
- `docs/src/mvp-acceptance.md` rewritten: three blocking items (manual walkthrough,
  no open data-loss bugs, CI release artifacts) replace the exhaustive manual
  checklist. Automated coverage is listed as a reference table. Known limitations
  (IME, no code signing, single document, no scroll-sync test) are documented
  transparently rather than treated as blocking gates.

## [0.7.0] - 2026-06-07

### Added
- **Word and character count** in the status bar: shows `N words` next to the
  save state; hovering reveals the character count. Counts are derived from
  `DocumentSession::word_char_count()` on the canonical text.
- **Template selector in Explorer** (RFC-037 UI): when `.bekoedit/templates/`
  contains `.md` files, a `<select>` dropdown appears in the new-file row.
  Choosing a template pre-fills the new file with that template's content via
  `AppState::create_from_template`.
- **RFC-002 bridge hardening**: `bridge::relay_js` centralises relay JS
  generation and embeds `BRIDGE_SCHEMA_VERSION` as `window.__bk_schema_version`
  so JS can detect contract mismatches at runtime.
- **Headless smoke test** (`--headless-smoke`): all five core paths
  (source preservation, filesystem, AppState open/edit/save, conflict
  detection, section operations) pass in a display-free environment.
  CI `.github/workflows/ci.yml` now runs `bekoedit --headless-smoke` as a
  post-build acceptance step.
- **CONTRIBUTING.md**: full developer guide covering prerequisites, build
  instructions, test requirements, quality gates, commit conventions, and
  the RFC process.
- **Documentation completion**: `docs/src/architecture.md` filled in with
  crate dependency graph, WebView boundary description, editing mode table,
  and filesystem safety notes.
- **Acceptance checklist evidence log** (`docs/src/mvp-acceptance.md`):
  every checklist item now has a status and evidence pointer; one item
  (IME composition) is marked ⚠️ for manual verification.

### Changed
- `DocumentSession` gains `word_char_count() -> (usize, usize)`.
- `StatusBar` updated: word count + line-ending + island count in one row.
- Explorer new-file row conditionally shows template selector when templates exist.
- `app.rs` uses `bridge::relay_js` instead of inline JS string literal.

[0.7.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.7.0

## [0.6.0] - 2026-06-07

### Added
- **Math rendering in preview** (RFC-038): `$inline$` and `$$block$$`
  LaTeX expressions are rendered as styled `<code class="math-inline">` /
  `<pre class="math-block">` elements showing the LaTeX source. KaTeX can
  be layered on top progressively once bundled without CDN dependency.
- **Footnote island classification** (RFC-038): `[^label]: text` footnote
  definitions are now indexed as `RawIslandType::Footnote` (previously
  `UnknownExtension`). The i18n label key `island.footnote` is included in
  both EN and JA tables.
- **Local document history**: on every successful save, a timestamped
  snapshot is recorded in the platform app-data directory (max 50 per
  document, oldest pruned automatically). The ⏱ History panel browses
  snapshots newest-first; "Restore" loads any snapshot as a new dirty
  edit without writing to disk.
- **RFC-032 performance benchmark**: `crates/bekoedit-markdown/benches/
  reparse.rs` measures full-reparse latency on a 240 KB synthetic document.
  Result: **3.57 ms/run** in release mode — the 50 ms threshold is not
  approached, confirming full-reparse-after-mutation is adequate.

### Changed
- `store.rs` split into six focused files: `store.rs` (core: 246 ELOC),
  `store_file_ops.rs`, `store_exports.rs`, `store_sections.rs`,
  `store_templates.rs`, `store_history.rs`. Every file is under 300 ELOC.
- `AppState` gains `list_history()` and `restore_history()`.
- `bekoedit_fs` exports `HistoryEntry` and `HistoryStore`.
- Preview CSS: `.math-inline` and `.math-block` styling added.
- History panel CSS: `.history-list`, `.history-entry`, `.history-time`.

### RFC-032 evaluation
Full-reparse-after-mutation confirmed adequate for current document sizes.
Incremental parsing deferred until profiling demonstrates a need.

[0.6.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.6.0

## [0.5.0] - 2026-06-07

### Added
- **Section move operations** (RFC-029): Up ↑ / Down ↓ buttons appear on
  each heading row in the Outline panel. Clicking swaps the section with its
  nearest sibling of the same heading level; sub-sections travel with their
  parent. The engine (`bekoedit_markdown::move_section_up/down`) touches only
  the swapped byte ranges — no other lines change.
- **Backlinks panel** (RFC-034): `bekoedit_fs::find_backlinks` scans all
  Markdown files under the workspace root for standard `[text](./path.md)` and
  wiki-style `[[page]]` references to the current document. Results open via
  the ⬡ button in the Editor Header; clicking a result opens that file.
- **Git status awareness** (RFC-036): `bekoedit_fs::git_status_map` runs
  `git status --porcelain` and returns a workspace-relative path→status map.
  The Explorer shows M (modified), A (added), D (deleted), ? (untracked)
  badges next to file names. Silently no-ops when Git is absent or the
  directory is not a repository.
- **Workspace templates** (RFC-037): `bekoedit_fs::list_templates` discovers
  `*.md` files under `.bekoedit/templates/` in the workspace root.
  `create_from_template` creates a new file pre-filled with the template text.
  `ensure_templates_dir` bootstraps the directory with a `blank.md` example
  on first use.

### Changed
- `OutlinePanel` updated: each heading row now shows ↑/↓ section-move buttons
  (hidden until hover to keep the panel uncluttered).
- `EditorHeader` updated: ⬡ (backlinks) toggle added between 🔍 (search) and
  ≡ (outline). Opening any panel closes the others.
- `AppState` extended with `move_section_up`, `move_section_down`,
  `list_templates`, `create_from_template`, and `git_status` methods.
- `bekoedit_fs` now exports `BacklinkEntry`, `find_backlinks`, `GitStatus`,
  `git_status_map`, `WorkspaceTemplate`, `create_from_template`,
  `list_templates`.
- `bekoedit_markdown` now exports `SectionError`, `SectionMoveResult`,
  `move_section_down`, `move_section_up`, `section_range`.

[0.5.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.5.0

## [0.4.0] - 2026-06-07

### Added
- **Inline formatting toolbar** (RFC-030): Bold, Italic, Code buttons appear above
  paragraph, blockquote, and heading fields in Form Mode. `onmousedown
  preventDefault` keeps textarea focus while JS reads `selectionStart/End` via
  a relay eval. A new `FormBlockEdit::ToggleInline` variant with UTF-16→UTF-8
  conversion wraps or unwraps markers in a single minimal source patch.
- **Simple GFM table editing** (RFC-027): Tables where all cells contain plain
  text are now classified as `BlockKind::SimpleTable` (form-editable) rather
  than `ComplexTable` raw islands. Form Mode renders them as interactive cell
  grids with per-cell inputs and an "+ Row" button. `FormBlockEdit::
  ReplaceTableCell` and `AddTableRow` apply minimal source patches, regenerating
  only the table block. Tables with inline formatting remain `ComplexTable` islands.
- **Image cards in Form Mode** (RFC-028): Image blocks render as a preview card
  with editable alt-text and path fields. `FormBlockEdit::ReplaceImage` rewrites
  only the `![alt](src)` markers.
- **Workspace full-text search** (RFC-033): `bekoedit_fs::search_workspace`
  scans all Markdown files under the workspace root, ranking exact-case matches
  above case-insensitive matches. A `SearchPanel` component (🔍 button) shows
  results with file path, line number, and a snippet; clicking a result opens the
  document.
- **HTML export** (RFC-035): `AppState::export_html` writes a self-contained
  HTML file (with inline CSS) to any path. The "Export HTML" button in the header
  exports alongside the current document.
- `bekoedit_markdown::utf16_to_utf8_offset` — public helper function for
  safe UTF-16 → UTF-8 position conversion; used by the inline toolbar bridge.

### Changed
- `form/resolve.rs` split into sub-modules: `inline_fmt`, `tables`, `images`.
- `form_tests.rs` split into `form_tests/basic_tests`, `inline_tests`, `table_tests`.
- `form_mode.rs` split into `form_mode/inline_toolbar`, `block_view`.
- `FormBlockDisplay` gains `Table { headers, rows, col_count }` and `Image { alt, src }` variants.
- `BlockKind` gains `SimpleTable` (form-editable) and `ComplexTable` (raw island)
  replacing the old single `Table` variant.

### Architecture decisions (RFC-031/032)
- **RFC-031**: Lexical not adopted. Custom projection approach (Form Mode as semantic
  patches against canonical Markdown) is retained as the correct fit for
  bekoedit's source-preservation invariants.
- **RFC-032**: Full-reparse-after-mutation confirmed adequate for current document
  sizes. Incremental parsing deferred until profiling demonstrates a need.

[0.4.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.4.0

## [0.3.0] - 2026-06-07

### Added
- **Native filesystem watcher** (RFC-005): `bekoedit-fs::FsWatcher` wraps
  `notify::RecommendedWatcher` (inotify/FSEvents/ReadDirectoryChangesW).
  External file modifications now trigger conflict detection and tree refresh
  within ~500 ms instead of relying solely on the autosave tick poll.
- **Split Mode** (RFC-010): side-by-side Text editor + rendered preview.
  Accessible via `Ctrl/Cmd+4` or the new Split tab in the mode switch.
  `EditorMode::Split` added to the `bekoedit-ui-contract` contract.
- **Outline panel** (RFC-010): heading navigation derived from the live
  `MarkdownIndex`. Clicking a heading scrolls CM6 to that position.
  Toggled with the `≡` button in the editor header (`Ctrl/Cmd+Shift+O`
  forthcoming). The panel is visible in all editing modes.
- **Scroll synchronisation** (RFC-012): in Split Mode, the preview pane
  mirrors the fractional scroll position of the CM6 editor via a JS
  scroll-event relay and a `dioxus::document::eval` call.
- **Outline toggle context** (RFC-010): `outline_open: Signal<bool>` added
  to app context; EditorHeader exposes the `≡` toggle button.
- **RFC integrity checker** (`scripts/check-rfcs.sh`): validates Status
  fields, `done/` completeness, duplicate numbers, and README link
  resolution — the optional CI invariant from RFC-000 §13.
- **CI smoke-test scaffold** (RFC-025): `build-and-smoke` job in CI builds
  the JS bundle, compiles the desktop binary, and runs a headless-launch
  check. The `--headless-smoke` flag is scaffolded as a no-op pending a
  small Dioxus startup probe.
- **Distribution docs** (RFC-024): `docs/src/distribution.md` covers
  Gatekeeper (macOS), SmartScreen (Windows), and apt deps (Linux).
- **MVP acceptance checklist** (RFC-026): `docs/src/mvp-acceptance.md`
  is the formal gate. Every criterion must be ticked before any v1.0 release.

### Changed
- `app.rs` background task: now drives `FsWatcher::drain()` each tick in
  addition to the autosave poll; the watcher is lazily started when a
  workspace opens and restarted if the workspace root changes.
- EditorHeader: outline toggle `≡` button added; mode switch includes Split.
- Keyboard shortcut `Ctrl/Cmd+4` mapped to Split mode.
- `AppState` context now includes a third `Signal<bool>` for outline
  panel visibility (explorer collapsed and settings open remain separate).

[0.3.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.3.0

## [0.2.0] - 2026-06-07

### Added
- **CodeMirror 6 Text Mode** (RFC-011): full CM6 editor with Markdown syntax
  highlighting, history, search panel, and tab-indent. The pre-built bundle
  (`assets/editor-bundle.js`) is committed so the app builds with no Node.js
  dependency at Cargo-build time. A bidirectional Dioxus eval relay synchronises
  text changes without going through the generic IPC channel.
- **Global keyboard shortcuts** (RFC-020): `Ctrl/Cmd+S` saves, `1/2/3` switches
  modes, `B` toggles the explorer. Shortcuts are handled by `assets/shortcuts.js`
  via an eval-bound relay that routes to the App-level Rust coroutine.
- **Accessibility baseline** (RFC-021): file tree has `role="tree"` / `role="treeitem"`
  with `aria-selected` and full keyboard navigation (Enter, F2 rename, Delete,
  Escape). Mode switch is a `role="tablist"`. StatusBar uses `role="status"` /
  `role="alert"`. All focusable elements have `:focus-visible` outlines.
- **Settings screen** (RFC-022): `AppSettings` (language, default mode,
  autosave delay, trash preference, reopen-last-workspace) persists atomically
  to the platform config directory. Live-applied on save without restart.
  `UserSettings` in `bekoedit-fs` covers workspace-level options shared by
  headless crates.
- **Toast notification system** (RFC-023): `Info/Success/Warning/Error` toasts
  with 4-second auto-dismiss and a polite ARIA live region. Explorer errors and
  save failures now surface as toasts instead of inline error text.
- `bekoedit-fs::UserSettings` — new module for workspace-scoped preferences.
- i18n tables expanded with 18 new keys in both English and Japanese.

### Changed
- Explorer: inline error text replaced by toasts; node actions now include
  an in-place rename input with Enter-to-commit and Escape-to-cancel.
- EditorHeader: settings button (`⚙`) opens the settings screen; explorer
  toggle (`☰`) collapses/expands the sidebar; save button pushes a success
  toast on completion.
- StatusBar: shows `LineEnding` variant alongside save state.
- Autosave: `create_app_state()` now reads autosave debounce from the
  platform default settings file.

[0.2.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.2.0

## [0.1.0] - 2026-06-07

### Added
- Source-preserving Markdown engine (`bekoedit-markdown`): full-reparse
  block index with exact UTF-8 byte ranges, revision-scoped block identity
  with content/context fingerprints, validated minimal source patches,
  Form Mode projection and semantic edit commands, Raw Markdown Island
  detection (front matter, HTML, tables, math, nested lists, complex
  blockquotes, malformed regions), style trivia capture (line endings,
  list markers, code fence style), sanitized preview rendering (document
  HTML escaped, scripts never execute).
- Filesystem services (`bekoedit-fs`): single-root workspaces with recent
  list, ignored-directory file tree index, traversal-rejecting path
  scoping, create/rename/delete-to-trash, atomic temp-and-rename saves,
  disk fingerprints, crash-recovery snapshots.
- Document core (`bekoedit-core`): document sessions with revisioned
  mutation paths (Text Mode snapshots, Form Mode semantic edits),
  debounced autosave scheduler, external-modification conflict detection
  and resolution (keep mine / reload / save copy), application state store.
- WebView boundary contract (`bekoedit-ui-contract`): versioned
  serializable commands and events; malformed payloads are recoverable.
- Desktop app (`bekoedit-app`): Dioxus Desktop shell with start screen,
  workspace explorer, Text/Form/Preview modes, conflict banner, status
  bar, and English/Japanese GUI i18n.
- Project documentation (mdBook-compatible `docs/src`), architecture
  invariants (`ARCHITECTURE.md`), RFC corpus under `rfcs/`, CI workflow.

[0.1.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.1.0
