# Appendix A: Source Preservation Glossary

## Canonical Source

The raw Markdown text loaded from disk and owned by the Rust document session. It is the only durable source of truth.

## Projection

A derived representation created from canonical source: preview HTML, outline, Form Mode blocks, parser index, file tree projection, or UI state.

## SourcePatch

A Rust-approved mutation to a UTF-8 byte range in canonical Markdown text. JavaScript does not author authoritative SourcePatch ranges.

## Raw Markdown Island

A region of Markdown that is not safe to edit visually. It is displayed as raw source or read-only preview with an escape hatch to Text Mode.

## BlockId

A revision-scoped logical identifier for a parsed block. It is used by Form Mode commands instead of authoritative byte ranges.

## Fingerprint

A hash or context marker used to validate that a UI projection still corresponds to the intended source block.

## Full Reparse

The MVP strategy of rebuilding the MarkdownIndex after every accepted mutation. This is safer than incremental parsing during early implementation.
