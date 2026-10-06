# Fixture corpus for `bekoedit-paste`

Pairs of `<name>.html` (the clipboard HTML) and `<name>.md` (the exact Markdown
`convert(…, LineEnding::Lf)` must produce). `tests/corpus.rs` also converts each
file with a CRLF target and requires the LF expectation with every `\n` replaced
by `\r\n`, so the pair covers both targets.

**Every fixture here is hand-written**, and its name starts `synthetic-`,
**except `webkitgtk-paste-conversion`**, the first real capture (below).
Nobody on this project can capture real clipboard HTML by hand: that needs a
person at a real application. Real captures from Firefox, Chromium, Google
Docs, Word and LibreOffice are future work (RFC-046 slice 1 §8), and when
they exist they get their own names, `<app>-…`, never `synthetic-…`. **Do not
describe a synthetic fixture as a capture, or a captured one as synthetic.**

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
| 16 | a Google-Docs-shaped paste: a non-bold `<b>` wrapper around blocks, and `font-weight:700` spans -- now recovered as bold (task 038) |
| 17 | a definition list |
| 18, 22 | literal `<` in prose |
| 19 | CR and CRLF inside the HTML itself |
| 20, 23, 24 | `data:` links and images, including nested in a link, and an SVG one with spaces, quotes and parentheses |
| 21 | a plain document, as a baseline |
| 25–29 | what `mdka` 3.0.0 changed from 2.5.1 (task 037): a superscript with a Unicode form (25) and without one, which gets a visible `^(…)` (26); a subscript, with `_(…)` where there is no Unicode form (27); a citation marker, which is left as it is (28); and emphasis that opens a bold element (29) |
| 30 | ordinals: `1<sup>st</sup>` becomes `1st` as of 3.2.0 (was `1ˢᵗ` under 3.0.0 and 3.1.1). **Flattened upstream in 3.2.0, at our suggestion** (`.git-exclude/upstream/mdka/send/2026-10-01-re-3.0.0-and-3.1.0.md`); confirmed against the published 3.2.0 binary in their reply, `.git-exclude/upstream/mdka/receive/2026-10-01c-shipped-you-can-turn-it-on.md` |
| 31–33 | more ordinal suffixes flattened in 3.2.0, each pinned from upstream's own worked examples: `1st 2nd 3rd 4th` (31); the Spanish ordinal marks `1º 2ª` (32); a suffix wrapped in a styled `<span>`, the shape a real editor's clipboard HTML uses (33) |
| 34, 35 | ordinal shapes 3.2.0 deliberately leaves alone, pinned as the boundary: French `1<sup>er</sup>` stays `1ᵉʳ` (a real exponent reads as an exponent, not a typographic ordinal); an italicised suffix, `1<sup><i>st</i></sup>`, still gives `1ˢᵗ` (a narrower, documented limit, a different code path than 33) |
| 40, 41 | **known gaps** (task 057 §2.2): a `<br><br>` run in a paragraph (40) and in a blockquote (41). The `.md` holds what the HTML means, two hard breaks written as a trailing `\`. Today the default two-space form writes a whitespace-only line, which CommonMark reads as a paragraph break, so every break in the run is lost, and in the quote the quote splits in two. Their damaging output is recorded in `tests/corpus.rs`'s `KNOWN_GAPS`, not typed in here |
| 42 | one `<br>` inside a list item, pinned as today's two-space form: a single break already works |
| 43 | `<br>` at the end of a paragraph: no break, since a break at the end of a block means nothing. Pinned as `text` |
| 44 | `<br>` inside a table cell: pinned as `x<br>y`, because the backslash option has no effect in a cell |
| 36–39 | `emphasis_from_style`'s own boundary, pinned from upstream's worked examples (task 038 step 4): a heading restating its own default `font-weight:700` is **not** emphasised (36); a `<cite>` restating its own default `font-style:italic` is **not** emphasised (37); a `<span style="font-weight:700">` authored *inside* a heading **is** emphasised -- an authored bold is real, only the heading's own restated default is ignored (38); and the one boundary upstream names and keeps deliberately, an **inherited** bold from a wrapping `<div>` still opens emphasis inside the `<h2>` beneath it (39) -- pinned as a known, accepted boundary, not a gap |

## `webkitgtk-paste-conversion`: the first real capture

RFC-046 slice 2 part B's `paste_conversion` release-checks scenario pasted a heading, a paragraph with a
tag-based `<b>bold</b>`, a two-item list and a 2×2 table, as a real X **CLIPBOARD** owner, into a real
WebKitGTK `paste` event, and logged the exact `text/html` the page received (merge push
`36826965725`, 2026-10-01). **WebKitGTK rewrites clipboard HTML before the page sees it**: the owner's
HTML was 84 characters for a simpler probe fixture (RFC-046 §3.0); this document's received form is
1,971 UTF-16 code units, because every element now carries its full computed inline style
(`caret-color`, `font-variant-caps`, `-webkit-tap-highlight-color`, and so on), and the space either side
of `<b>bold</b>` arrives wrapped in `<span class="Apple-converted-space">` holding a literal **U+00A0**
non-breaking space, not an ASCII one.

**Under today's configuration (3.0.0, `Minimal`), nothing is lost.** The heading, the bold, the list and
the table all survive; the two U+00A0 characters are read as plain spaces.

**What each element's own `style` actually carries here, checked directly rather than assumed:** `<p>`,
`<ul>` and `<table>` each restate `font-weight: 400` (their own normal weight); **`<h1>` carries no
`font-weight` at all**, and **`<th>`/`<td>` carry no `style` attribute at all** — only `<p>`/`<ul>`/`<table>`
do. So the slice 2 handoff's worry (RFC-046 §6.2 item 4: a heading or a table header cell arriving with a
computed `font-weight: 700` that a style-reading option would then wrap in `**…**`) does **not** describe
*this* captured document: nothing here restates a bold default on a heading or a cell for such an option
to misread, because WebKitGTK did not attach one. Task 038 confirms this directly against the real
`emphasis_from_style` option rather than relying on this observation alone — a different real paste could
still carry it.

## Accepted limitations, recorded as expectations (RFC-046 §6.2)

- **11, 12, 17:** what has no Markdown form becomes one paragraph per cell, term or
  description. The text is kept.
- **The `<div>`-inherits-into-`<h2>` boundary (39):** an ancestor's bold opens emphasis
  inside a heading beneath it, where the heading's own restated default does not.
  Upstream names this deliberately (`.git-exclude/upstream/mdka/receive/2026-10-01c-shipped-you-can-turn-it-on.md`
  §3) and does not expect it in a computed-style paste. **Not a gap to close**, but
  recorded so a future change to it is seen.

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

## What `mdka` 3.0.0 moved from 2.5.1 (task 037)

Upstream predicted two output changes, both fixes: `<sup>` and `<sub>` keep their meaning (2.6.0), and
emphasis that opens a bold element is kept (2.7.0). **No fixture from 01 to 24 moved.** Fixtures 25 to 29 were
written from upstream's letter, by hand, and **each of 25, 26, 27 and 29 fails under 2.5.1** (which is what makes
them pin the new behaviour); 28 passes under both, because a citation marker is untouched.

Two details, checked by running both versions:

- The letter's `10−9` reproduces on 2.5.1 only with a real minus sign, **U+2212**, in the source
  (`<sup>−9</sup>`). With an ASCII hyphen (`<sup>-9</sup>`), 2.5.1 already gave `10⁻⁹`. Fixture 25 uses U+2212.
- 2.5.1 and 3.0.0 both write a citation marker as `\[1]`: only the opening bracket is escaped.

No known upstream gap is recorded in this corpus: every expectation above passes.

## What `mdka` 3.2.0 moved from 3.0.0 (task 038)

The pin moved `=3.0.0` → `=3.2.0`, mode and options unchanged (`Minimal`, no `emphasis_from_style`).
Upstream measured 3.0.0 against 3.1.1 as 108-comparison byte-identical with the option off, and 3.1.1 →
3.2.0 as ordinals-only; running every fixture here, including `webkitgtk-paste-conversion`, against the
real 3.2.0 binary confirms it independently: **only fixture 30 moved**, exactly as predicted, and every
other fixture — 01 through 29, and the real capture — is still byte-identical. Fixtures 31 to 35 are new,
pinning the rest of upstream's stated 3.2.0 behaviour by hand from their letter, each run against the real
binary before being typed in (`.git-exclude/upstream/mdka/receive/2026-10-01c-shipped-you-can-turn-it-on.md`).

## `emphasis_from_style` is on (task 038 step 4)

`mdka_configured` now calls `.emphasis_from_style(true)`: bold or italic carried only by an element's own
inline `style` (`font-weight` ≥ 600 or `bold`; `font-style: italic`/`oblique`) is read, not just tag-based
`<b>`/`<strong>`/`<i>`/`<em>`. Every fixture was converted with the option on and compared against its
existing expectation before anything was changed, so the result below is exhaustive over this corpus, not a
sample:

- **Two fixtures moved, both (a) wanted:**
  - **04**, `<span style="font-weight:700">B</span> and <span style="font-style:italic">I</span>` →
    `**B** and *I*` (was `B and I`). Exactly upstream's item 4.
  - **16**, the Google-Docs-shaped paste → `# **Title**` and `**bold-only-by-style**` kept (was lost). The
    title's bold comes from a `<span>` *inside* the `<h1>`, not the `<h1>`'s own style -- the distinction
    fixture 38 pins directly.
- **Nothing else moved**, including `webkitgtk-paste-conversion`: its `<h1>`, `<th>` and `<td>` carry no
  `font-weight` in their own style at all (recorded above), so there was nothing for the option to find
  there, spurious or otherwise, in this particular real capture.
- **Fixtures 36 to 39** (above) pin upstream's own worked boundary examples by hand, each run against the
  real binary first. **No spurious case was found**: no heading, table header cell, or already-bold element
  anywhere in this corpus gained an unwanted `**…**` from a restated or inherited default, except the one
  boundary (39) upstream names and keeps deliberately.

Since every move was (a) and no fixture showed (b), the option stays on. The CHANGELOG's paste entry no
longer lists style-only bold as a limit.


## `mdka` 3.3.0, and `backslash_hard_breaks` (task 057)

The pin moved `=3.2.0` → `=3.3.0`, mode and options unchanged. `Cargo.lock` changed `mdka` alone.

- **Byte-identical to 3.2.0.** Every existing fixture (01 to 39, `webkitgtk-paste-conversion`) was converted by a copy of this crate pinned to `=3.2.0` and by this one at `=3.3.0`, and the complete outputs diffed: 41 fixtures, no difference. Upstream's own comparison (0 of 108) is not evidence for this corpus on its own, since the `.md` expectations were hand-written, not produced by 3.2.0.
- **`backslash_hard_breaks` stays off.** Turning it on (`.backslash_hard_breaks(true)` in `mdka_configured`) was measured against every fixture. Four outputs move:
  - **40** and **41** moved to exactly their hand-typed expectations: a break kept that was lost. Both are wanted.
  - **09** (`line one  \nline two  \nline three`) becomes `line one\` / `line two\` / `line three`, and **42** (`- first  \n  second`) becomes `- first\` / `  second`. A break was kept both times, only its form changed, so neither is a break saved. Those two are otherwise, which stops the switch.
  - **43** and **44** do not move.
- **The option was not enabled.** The two form-only changes are a visible change to every hard break already pasted, which the task classifies as "otherwise". Gaps 40 and 41 stay recorded in `KNOWN_GAPS` until the owner decides.
