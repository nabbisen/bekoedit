# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.16.1] - 2026-09-25

### Highlights
- Security: clicking a link in Preview now opens only web and email links.
  Other links, including relative paths and network paths such as
  `\\host\share`, no longer reach your operating system. In earlier versions
  on Windows, clicking a network-path link could make the computer contact
  another machine over the network.
- Security: Preview no longer shows `javascript:` and similar links as
  clickable.
- A `<br>` in a table cell now shows as a line break in Preview.

### Security
- Preview no longer renders `javascript:` and similar link destinations as live
  links. Links and images now pass an allowlist (`http:`, `https:`, relative
  paths and `#fragments`, `mailto:` for links, and PNG, JPEG, GIF and WebP
  `data:` images); anything else, including network paths such as `//host/x`
  and `\\host\share`, is shown as its plain text and is never clickable. Your Markdown file is not changed.
- Clicking a link in Preview no longer hands a path to the operating system.
  Only web (`http:`, `https:`) and email (`mailto:`) links open, in your
  browser or mail client. A relative link such as `other.md`, or a
  `#section` link, used to be passed to the system's opener as a path relative
  to the folder bekoedit was started from; it now opens nothing outside the
  app. A link that is not followed shows a short notice saying why. Following
  a link to another document from Preview is not supported yet.

### Fixed
- A `<br>` inside a line, for example in a table cell, now shows as a line break
  in Preview instead of as the literal text `<br>`.

## [0.16.0] - 2026-09-25

### Highlights
- Reopen your last workspace on launch (turn it on in Settings).
- Text Mode keeps Windows (CRLF) line endings; earlier versions could rewrite
  them as LF on save or on a mode switch.
- Windows: no console window at startup, and a clear message if the WebView2
  Runtime is missing.
- Focus lands in the editor after opening from the tree, a backlink, search or
  a menu, and clicks made while the editor switches modes are no longer lost.
- Release archives include the platform helper script.
- Security: `rustls` updated (RUSTSEC-2026-0285).

### Added
- **Reopen last workspace on launch.** The Settings checkbox of that name now
  works: when enabled, bekoedit opens your most recent workspace before the
  first frame, with no Start Screen flash. It opens a workspace, never a
  document, so unsaved-change recovery still takes precedence. If the most
  recent workspace cannot be opened — an unmounted volume, a deleted folder —
  bekoedit shows the Start Screen with a notice naming it, and never silently
  opens a different project instead. The setting has existed since v0.2.0 and
  did nothing until now (RFC-043).
- **Platform helper scripts now ship inside the release archives.** Each
  archive contains its own platform's script at the root, beside the binary:
  `run-linux.sh`, `run-macos.sh` or `run-windows.ps1`. The first-run
  instructions in `README.md` have referenced these since v0.8.0, but no
  archive contained them — most sharply on macOS, where `run-macos.sh` clears
  the quarantine attribute and is the entire first-run path for an unsigned
  binary (RFC-045).

### Fixed
- Opening a document from the workspace tree or a backlink now moves focus to
  the editor.
- Opening a search result in Text or Split mode, and choosing "New File" or
  "Split" from a menu, now move focus to the editor. The two menu items no
  longer leave later focus requests refused.
- Opening the application menu or the editor tools menu from the keyboard
  (Down, Up, Enter or Space on its button) now moves focus into the menu.
  Before this fix, focus stayed on the button, so the arrow keys could not
  reach the items. 0.14.0 and 0.15.0 both listed this as working.
- On Windows, starting bekoedit no longer opens a console window next to the
  app.
- On Windows, if the Microsoft Edge WebView2 Runtime is missing, bekoedit now
  says so and links to the download, instead of closing immediately.
- Clicking a mode tab while the editor is still switching now moves focus into
  the editor, as an ordinary click does.
- A click or keystroke that arrives while the editor is still switching is no
  longer ignored. It runs as soon as the editor is ready, or, if it cannot, a
  message says which action did not happen and why. Saving is never applied to
  a different document than the one that was open when you asked.
- Text Mode no longer converts Windows (CRLF) line endings to Unix ones. Before
  this fix, a file with CRLF line endings was rewritten with LF endings when you
  saved it after an edit, and also when you switched between modes without
  editing anything at all (autosave then wrote the change). Now every line you
  did not touch keeps its own ending: files with one consistent line ending
  keep it on every line, and in files that mix endings an edit keeps the
  endings of the lines it does not touch (see *Source preservation* in the
  documentation for one limit on replace-all). The outline panel also now
  jumps to the right heading in documents that contain CRLF line endings or
  non-ASCII text.

### Security
- Updated `rustls` to 0.23.45, resolving RUSTSEC-2026-0285.

## [0.15.0] - 2026-08-17

### Added
- Accessibility metadata for Form Mode block editing: every block is a named
  group stating its kind, and raw Markdown islands state both that they are
  verbatim regions and why they could not be edited as a form. This completes
  RFC-042, whose remaining gap 0.14.0's notes recorded explicitly.

### Security
- Updated the transitive `webbrowser` dependency from 1.2.1 to 1.2.4, resolving
  RUSTSEC-2026-0257 (Unix `BROWSER` handling allows browser argument injection).
  The crate is reached only through `dioxus-desktop`; no bekoedit code opens
  external links, and exploitation requires control of the user's `BROWSER`
  environment variable. Selected in `Cargo.lock` under the existing upstream
  requirement — no manifest pin was added.

## [0.14.0] - 2026-08-10

### Added
- Full keyboard navigation for the workspace file tree — arrow keys, Home/End,
  Enter and Space — with the tree as a single tab stop and the open document
  announced as the selected row.
- Keyboard navigation for the application and editor-tools menus and for the
  editor mode tabs, following the WAI-ARIA menu-button and tabs patterns.
- Accessibility roles, names, and announcements for the conflict banner, the
  Recovery screen, and the Settings screen. Recovery announces how many
  documents can be recovered.
- A warning when settings cannot be written to the platform configuration
  directory and fall back to temporary storage, where they may not survive a
  restart.

### Changed
- Keyboard focus is arbitrated by a single authority shared between the
  application shell and the source editor, replacing two independent owners
  that could each move focus without the other's knowledge.
- Settings failures are reported instead of discarded, and the Settings screen
  stays open when a save fails so that edits are not lost.

### Fixed
- The split divider no longer displays a resize cursor for a drag it cannot
  perform.

### Removed
- `bekoedit-fs`: `UserSettings::default_path`, `UserSettings::load`,
  `UserSettings::save`, `load_user_settings`, and `save_user_settings`. None had
  a caller; the `UserSettings` type itself is unchanged.
- `bekoedit-ui-contract`: the unused `UiToCoreCommand` and `CoreToUiEvent`
  payload types, and with them the crate's dependencies on `bekoedit-core`,
  `bekoedit-fs`, and `bekoedit-markdown` — it is now a leaf crate.
  `BRIDGE_SCHEMA_VERSION`, `EditorMode`, and the `source_editor` protocol are
  unchanged.

Both removals are breaking for library consumers, which is why this is a minor
release rather than a patch.

Accessibility work in this release covers the shell: file tree, menus, mode
tabs, conflict banner, Recovery, and Settings. **Form Mode block editing does
not yet expose accessibility metadata** and is tracked by RFC-042 slice 5.

[Unreleased]: https://github.com/nabbisen/bekoedit/compare/0.16.1...HEAD
[0.16.1]: https://github.com/nabbisen/bekoedit/releases/tag/0.16.1
[0.16.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.16.0
[0.15.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.15.0
[0.14.0]: https://github.com/nabbisen/bekoedit/releases/tag/0.14.0

## [0.13.1] - 2026-07-21

### Changed
- Updated transitive `anyhow` from 1.0.102 to 1.0.103 through the Dioxus
  core/desktop and image/rav1e dependency paths, resolving
  RUSTSEC-2026-0190.
- Updated transitive `memmap2` from 0.9.10 to 0.9.11 through Dioxus and
  `subsecond`, resolving RUSTSEC-2026-0186.
- Both patched releases are selected in `Cargo.lock` under their upstream
  semver-compatible requirements; no direct application dependency or exact
  manifest pin was added.
- This closes the two time-bounded 0.13.0 patch-level deferrals tracked by
  `AUDIT-2026-07-17-01`; its remaining dependency workstreams stay open.
- Declared the already-effective Rust 1.88 floor for the application, core, and
  Markdown packages, added exact lower-bound CI, and aligned source-build
  documentation without overriding contributors' selected compiler.
- Hardened CI and release workflows with immutable action commits, locked Cargo
  resolution, read-only target-bound builders, and a single write-authorized
  publication job.
- Added shared verification for release archive names, checksums, safe root
  members, target provenance, and complete cross-platform artifact sets.

[0.13.1]: https://github.com/nabbisen/bekoedit/releases/tag/0.13.1

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

Older releases:

- [0.1 to 0.9](changelog/0.1-0.9.md)
