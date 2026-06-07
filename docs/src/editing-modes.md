# Editing Modes

One document, three projections. Switch with the header buttons; the text
itself never changes on switch.

## Text Mode

The raw Markdown source. Changes are revision-checked snapshots: what you
see is exactly the canonical text.

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

### Raw Markdown Islands

Front matter, HTML blocks, tables, math, nested or multi-paragraph lists,
complex blockquotes, and malformed regions appear as highlighted raw-text
regions with a label explaining why. You edit them verbatim — bekoedit
never reinterprets or normalizes them.

## Preview Mode

Read-only rendering of the document. Raw HTML in your Markdown is shown
escaped, never injected — scripts in documents cannot execute.
