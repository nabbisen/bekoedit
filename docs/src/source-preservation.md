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

## Text Mode and line endings

The editor always shows a document with `\n` line breaks, whatever the file
contains. The session keeps the file's own bytes and reconciles the editor's
text with them (`bekoedit-core`, `editor_text.rs`):

- Text that is just the editor's view of the document is not an edit. Nothing
  changes: no revision, no unsaved-change marker, no write.
- In a file whose line endings are all `\n`, or all `\r\n`, every new line
  break takes the file's ending.
- In a file with mixed endings, or any lone `\r`, only the span between the
  first and last changed character is replaced; every byte outside it is
  untouched, and a line break you type takes the ending of the line it splits.

**Known limit.** In a *mixed* file, one edit changes only the lines it touches.
A single change delivered at several separated places at once (for example a
replace-all) can re-end the untouched lines *between* them, because the span
runs from the first change to the last.
