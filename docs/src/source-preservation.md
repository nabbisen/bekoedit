# Source Preservation Model

The engine never round-trips your document through an AST. Instead:

1. **Index**: a full parse maps each top-level block to its exact UTF-8
   byte range in the source, plus a content range and *style trivia*
   (line ending, list marker style, fence character/length).
2. **Identity**: each block gets a `BlockId` — document revision, ordinal,
   kind, and a fingerprint of its content and surrounding context. Stale
   commands (wrong revision or fingerprint) are rejected and the UI
   refreshes; they can never patch the wrong bytes.
3. **Patches**: semantic edits resolve to a `SourcePatch{range,
   replacement}` that is validated (bounds, char boundaries) and replaces
   only the targeted bytes.
4. **Islands**: anything the engine can't edit safely — front matter,
   HTML, tables, math, nested lists, malformed syntax — is surfaced as a
   Raw Markdown Island and only ever edited verbatim.
5. **Reparse**: after each mutation the whole document reparses (the MVP
   simplicity rule); incremental parsing is a deferred optimization.

The golden test suite locks this in: editing one block of a document
containing CRLF, Japanese text and emoji, mixed list markers, tilde
fences, non-1 ordered lists, reference links, front matter, HTML, and
tables must leave every other byte identical.
