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
