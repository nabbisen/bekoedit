# Roadmap

Authoritative sequencing lives in `rfcs/`. This file is the orientation view.
**v1.0.0 requires explicit maintainer sign-off before any release.**

## Remaining proposed RFCs

- **RFC-038** — advanced Markdown extension policy (GFM footnotes, math rendering, custom directives)
- **RFC-031** — decided (no Lexical; see rfcs/proposed/)
- **RFC-032** — deferred until profiling shows need
- **RFC-039/040** — future evaluation only

## Archive (shipped earlier, ROADMAP was stale)

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

## Shipped — v0.5.0 (2026-06-07)
Section move (RFC-029), backlinks (RFC-034), Git awareness (RFC-036), workspace templates (RFC-037).

## Shipped — v0.4.0 (2026-06-07)
Inline formatting toolbar, simple GFM table editing, image cards, workspace search, HTML export. RFC-031 Lexical decision written (decision: retain custom approach).

## Shipped — v0.3.0 (2026-06-07)
Native filesystem watcher (inotify/FSEvents/ReadDirectoryChangesW),
Split Mode with scroll-sync, Outline panel, distribution docs, CI
smoke-test scaffold, MVP acceptance checklist. All MVP-critical RFCs
(000–026) are now either Implemented or deferred to post-MVP.

## Shipped — v0.2.0 (2026-06-07)
CodeMirror 6 Text Mode, global keyboard shortcuts, ARIA accessibility baseline,
settings persistence + UI, toast error surfaces.

## Shipped — v0.1.0 (2026-06-07)
Source-preserving engine, filesystem safety, sessions/save/conflicts,
WebView contract, desktop shell with Text/Form/Preview, i18n (en/ja).
