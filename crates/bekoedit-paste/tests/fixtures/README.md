# Fixture corpus for `bekoedit-paste`

Pairs of `<name>.html` (the clipboard HTML) and `<name>.md` (the exact Markdown
`convert(…, LineEnding::Lf)` must produce). `tests/corpus.rs` also converts each
file with a CRLF target and requires the LF expectation with every `\n` replaced
by `\r\n`, so the pair covers both targets.

**Every fixture here is hand-written**, and its name starts `synthetic-`. Nobody
on this project can capture real clipboard HTML: that needs a person at a real
application. Real captures from Firefox, Chromium, Google Docs, Word and
LibreOffice are future work (RFC-046 slice 1 §8), and when they exist they get
their own names. **Do not describe a synthetic fixture as a capture.**

The `.md` files were written by hand from what correct Markdown should be, not
copied from the converter's output.

## Bytes are kept as written

`.gitattributes` marks this directory `-text`, so a checkout never rewrites a line
ending. `synthetic-19` holds real `\r\n` bytes on purpose.

## What each group is

| Fixtures | What they check |
|---|---|
| 01–09 | our nine reproductions sent to upstream on 2026-09-16, one each (tables; `1\.` escaping; Google Docs' wrapper around blocks; inline-style emphasis; strikethrough; task lists; `data:` images; `id` anchors; hard breaks) |
| 10 | upstream's `2.4.1` regression, verbatim: a table cell holding two sibling `<div>`s. Fixed in `2.4.2` |
| 11–14 | tables with no GFM form (row headers; nested) and tables inside a blockquote and a list item, each with `<br>` in a cell |
| 15 | `Vec<i32>` in inline code and in a fenced block |
| 16 | a Google-Docs-shaped paste: a non-bold `<b>` wrapper around blocks, and `font-weight:700` spans |
| 17 | a definition list |
| 18, 22 | literal `<` in prose |
| 19 | CR and CRLF inside the HTML itself |
| 20, 23, 24 | `data:` links and images, including nested in a link, and an SVG one with spaces, quotes and parentheses |
| 21 | a plain document, as a baseline |

## Accepted limitations, recorded as expectations (RFC-046 §6.2)

- **04 and 16:** bold or italic carried only by an inline `style` (`font-weight:700`,
  `font-style:italic`) is lost. The text is kept. This is upstream's item 4, and it
  is what a Google Docs paste looks like. **The expectation is the accepted
  behaviour, not the ideal one.**
- **11, 12, 17:** what has no Markdown form becomes one paragraph per cell, term or
  description. The text is kept.

## Upstream's claims (RFC-046 §6.2), and where each is checked

- *In `Minimal`, the only raw HTML is `<br>`, and only in cells:* the structural
  test in `tests/corpus.rs`, over every fixture.
- *The 2.4.1 wrapper-in-cell bug is fixed:* fixture 10.
- *`<br>` outside a table becomes two trailing spaces:* fixture 09.
- *A literal `<` in prose is escaped:* fixtures 18 and 22. **It holds where it
  matters**: every `<` that would start a tag, comment, processing instruction or
  autolink is escaped (`\<a`, `\</b>`, `\<!--`, `\<?php`, `\<https://…>`,
  `\<me@example.com>`). It does not hold literally for every `<`: `x < y` and `<3`
  stay as they are, and they cannot start markup.

No known upstream gap is recorded in this corpus: every expectation above passes.
