# Roadmap

Authoritative sequencing lives in `rfcs/`. This file is the orientation view.
**v1.0.0 requires explicit maintainer sign-off before any release.**

## Remaining proposed RFCs

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

## Remaining proposed RFCs
- **RFC-031**: Lexical decision (decided: no adoption — see rfcs/proposed/)
- **RFC-032**: Incremental parsing (deferred — profiling shows no need yet)
- **RFC-039/040**: Plugin system, sync/collaboration — future evaluation only

**v1.0.0 requires explicit maintainer sign-off on the acceptance checklist.**

## Shipped — v0.9.0 (2026-06-07)
Recovery screen (RFC-007 UI), large-file warning, relay auto-restart
(RFC-002), `file_size_bytes` query. All acceptance checklist items have
code evidence. Only the human walkthrough and IME manual check remain.

## Shipped — v0.8.0 (2026-06-07)
IME composition guard in CodeMirror 6 (RFC-011), User-facing error messages for all `StoreError` and `FileOpError` variants, Settings persistence helpers, Recent-workspaces persistence test, Large workspace stress test, Platform scripts, Production README, Scroll-fraction reporter. Codebase housekeeping.

## Shipped — v0.7.0 (2026-06-07)
v1.0.0 preparation: word/char count, template selector UI, RFC-002 bridge
hardening, headless smoke test, CONTRIBUTING.md, docs completion, acceptance
checklist evidence log. All automated checks green; IME manual verification
pending.

## Shipped — v0.6.0 (2026-06-07)
Math/footnote extension policy (RFC-038), local document history, RFC-032 performance evaluation (3.57 ms/reparse — threshold not approached), store.rs split (all files ≤300 ELOC).

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
