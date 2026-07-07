# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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

[0.1.0]: https://github.com/nabbisen/bekoedit/releases/tag/v0.1.0

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

[0.2.0]: https://github.com/nabbisen/bekoedit/releases/tag/v0.2.0

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

[0.3.0]: https://github.com/nabbisen/bekoedit/releases/tag/v0.3.0

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

[0.4.0]: https://github.com/nabbisen/bekoedit/releases/tag/v0.4.0

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

[0.5.0]: https://github.com/nabbisen/bekoedit/releases/tag/v0.5.0
