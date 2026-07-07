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
