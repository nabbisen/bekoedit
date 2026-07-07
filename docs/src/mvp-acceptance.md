# MVP Acceptance Checklist

This document is the formal gate for the bekoedit v1.0 release.
**No v1.0 release may happen without explicit maintainer sign-off on
every item below.**

---

## Source preservation (RFC-013/014/015)

- [ ] Golden test suite passes: editing one block of the adversarial
  document (CRLF, Japanese/emoji, mixed markers, tilde fences, non-1
  ordered lists, reference links, front matter, HTML, tables) leaves
  every other byte unchanged.
- [ ] Form Mode edits on a 10 000-word document produce patches targeting
  only the edited region (benchmark: <50 ms round-trip on reference hardware).
- [ ] UTF-8 boundary patches are impossible; all attempted invalid-boundary
  patches are rejected with structured errors (not panics).
- [ ] Raw Markdown Islands are never silently normalized or deleted.

## Document lifecycle (RFC-006/007/008)

- [ ] Autosave writes atomically; killing the process mid-save leaves the
  file either fully written or untouched.
- [ ] External modifications are detected within 1 s (fs watcher) or 500 ms
  (polling fallback) of occurrence.
- [ ] Neither the disk version nor the in-memory version is silently lost
  when a conflict is detected.
- [ ] A crash-recovery snapshot exists before any risky write, and is
  presented clearly on next launch.
- [ ] Save failures surface to the user and keep the document dirty.

## File operations (RFC-003/004/005)

- [ ] Path traversal outside the workspace root is rejected for all file
  operations (create, rename, delete, open).
- [ ] Deletion goes to the system trash by default; permanent deletion
  requires a second confirmation.
- [ ] Renaming the open document updates the session path without data loss.
- [ ] Deleting a dirty open document is blocked until the user resolves it.

## Editing modes (RFC-010/011/016/019)

- [ ] Switching modes does not alter the canonical Markdown source.
- [ ] Text Mode (CodeMirror 6) correctly handles Japanese IME composition
  without garbling multibyte text.
- [ ] Form Mode sends only semantic commands; whole-document rewrite is
  impossible from Form Mode.
- [ ] Split Mode scroll synchronisation tracks the editor scroll position
  in the preview pane.
- [ ] Outline panel navigation scrolls CM6 to the correct heading.

## Accessibility (RFC-021)

- [ ] All primary workflows (open workspace, open file, edit, save, switch
  mode, rename, delete) are completable using keyboard only.
- [ ] File tree exposes `role="tree"` / `role="treeitem"` with correct
  `aria-selected` and keyboard navigation (arrows, Enter, F2, Delete).
- [ ] Save status changes are announced via the polite live region.
- [ ] Save failures are announced via the assertive live region.
- [ ] All interactive elements have a visible `:focus-visible` outline.

## Internationalisation (i18n)

- [ ] All user-visible strings have entries in both the English and
  Japanese tables; no key falls through to the bare key string.
- [ ] Japanese workspace paths and document content (including emoji) load
  and save correctly.

## Distribution (RFC-024)

- [ ] CI produces artefacts for Linux (x86_64), macOS (aarch64), and
  Windows (x86_64).
- [ ] Each artefact includes README, LICENSE, NOTICE, and CHANGELOG.
- [ ] SHA-256 checksums are published alongside artefacts.
- [ ] Platform installation notes (SmartScreen / Gatekeeper / apt deps)
  are published in the documentation.

## CI quality gates (RFC-025)

- [ ] `cargo fmt --all` passes (no formatting divergence).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes on all three target platforms.
- [ ] The desktop binary builds and launches without crashing on each
  target platform (smoke test in CI).
- [ ] No source file exceeds 500 effective lines of code.

## Documentation

- [ ] README covers: project purpose, architecture note, build instructions
  (including WebView deps on Linux), running tests.
- [ ] `docs/src` mdBook chapters cover: getting started, editing modes,
  saving/conflicts, architecture overview.
- [ ] CHANGELOG has entries for every released version.
- [ ] ROADMAP is up to date with shipped and upcoming work.
- [ ] `ARCHITECTURE.md` invariants match the implemented behaviour.

---

*Last updated: 2026-06-07. Maintainer sign-off required before v1.0 release.*
