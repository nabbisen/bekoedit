# Roadmap

Authoritative sequencing lives in `rfcs/`. This file is the orientation view.
**v1.0.0 requires explicit maintainer sign-off before any release.**

## Shipped — v0.2.0 (2026-06-07)
CodeMirror 6 Text Mode, global keyboard shortcuts, ARIA accessibility baseline,
settings persistence + UI, toast error surfaces.

## Shipped — v0.1.0 (2026-06-07)
Source-preserving engine, filesystem safety, sessions/save/conflicts,
WebView contract, desktop shell with Text/Form/Preview, i18n (en/ja).

## Next (remaining MVP proposed RFCs)

- **RFC-005** — native filesystem watcher (inotify / FSEvents /
  ReadDirectoryChanges) replacing the current poll-on-tick model.
- **RFC-010** — outline panel tab, resizable sidebar panes, split-pane
  Text+Preview mode.
- **RFC-012** — preview scroll-synchronisation with the editor.
- **RFC-024/025** — packaging scripts, unsigned binary guidance, CI
  smoke-test suite (launch + basic workflow).
- **RFC-026** — MVP acceptance matrix sign-off and beta readiness review.

## Post-MVP (RFCs 027–032)
Table editing, image/asset management, outline operations, richer inline
formatting, Lexical integration decision, incremental parsing.

## Future evaluation (RFCs 033–040)
Full-text search, backlinks, export profiles, Git awareness, workspace
templates, extension policy, plugin system, sync/collaboration.

## Shipped — v0.3.0 (2026-06-07)
Native filesystem watcher (inotify/FSEvents/ReadDirectoryChangesW),
Split Mode with scroll-sync, Outline panel, distribution docs, CI
smoke-test scaffold, MVP acceptance checklist. All MVP-critical RFCs
(000–026) are now either Implemented or deferred to post-MVP.
