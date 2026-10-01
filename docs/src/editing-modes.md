# Editing Modes

One document, three projections. Switch with the header buttons; the text
itself never changes on switch.

## Text Mode

The raw Markdown source. Changes are revision-checked snapshots: what you
see is exactly the canonical text.

### Pasting formatted text

Pasting (Ctrl+V, Cmd+V on macOS) formatted content from a browser or an
office app converts it into Markdown, as one undo step: headings,
paragraphs, bold and italic — including bold or italic carried only by a
style, as Google Docs uses — links, lists, task lists, strikethrough, code,
and tables.

**Ctrl+Shift+V** (Cmd+Shift+V) pastes plain text, unconverted.

Formatted content pastes as plain text instead, with a notice, when:

- it is too large to convert (over 1 MiB) — *"Pasted content was too large
  to convert; pasted as plain text."*
- conversion fails — *"Pasted content could not be converted; pasted as
  plain text."*
- conversion takes too long (over 2 seconds) — *"Pasted content took too
  long to convert; pasted as plain text."*

A table shaped so it has no Markdown form — a header cell in every row, two
header rows, a nested table, or a caption — arrives as plain paragraphs
instead, with a notice: *"The pasted table could not be kept as a Markdown
table; its text was kept."* The text is always kept; only the grid can be
lost.

An image embedded in the page (a `data:` image) arrives as its description
— the image's alt text — or a short marker, *"image"*, if it has none.

A paste that arrives after you have switched to a different document is not
applied, and a notice says so: *"Paste not applied: the document changed
before it was ready."*

These notices exist in Japanese too.

Your file is never changed beyond the pasted text, and keeps its own line
endings — see [Source Preservation Model](source-preservation.md).

**Known limit:** bold declared on a block that wraps a heading also makes
the heading bold. This is a deliberate choice in the converter upstream,
not a bekoedit decision.

## Form Mode

Each block renders as a typed control:

| Block | Control |
|-------|---------|
| Heading | level selector + text field (setext headings keep their level) |
| Paragraph | multi-line field |
| Bullet / ordered list | one field per item — markers and numbering style preserved |
| Task list | checkbox + field; toggling patches exactly one character |
| Fenced code | language field + code area — fence character and length preserved |
| Simple blockquote | multi-line field |
| Horizontal rule | shown as a rule; deletable |

Edits commit when a field loses focus (or on Enter), producing one
minimal patch. Each block has a delete button; deletion also removes the
trailing blank lines so no gaps accumulate.

Pasting into a field always inserts plain text; formatted content from the
clipboard is not converted here, unlike in Text Mode above.

### Raw Markdown Islands

Front matter, HTML blocks, tables, math, nested or multi-paragraph lists,
complex blockquotes, and malformed regions appear as highlighted raw-text
regions with a label explaining why. You edit them verbatim — bekoedit
never reinterprets or normalizes them.

## Preview Mode

Read-only rendering of the document. Raw HTML in your Markdown is shown
escaped, never injected — scripts in documents cannot execute.

### Links

- Only `http:`/`https:` links and `mailto:` links open, in your browser or
  mail client.
- A relative link (such as `other.md`) does not open; a notice says why.
- A network path (such as `//host/x` or `\\host\share`) is shown as plain
  text, not as a link.
- Following a link to another document from Preview is not supported yet.
- A `#` link stays on the page.
- A link with another scheme, such as `javascript:`, is shown as its plain
  text, not as a clickable link.
- A `<br>` inside a line — for example in a table cell — shows as a line
  break, not as literal text.
