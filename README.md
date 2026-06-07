# bekoedit

A source-preserving Markdown editor. Edit Markdown visually — headings,
paragraphs, lists, tasks, and code as real form controls rendered in a Web
DOM — while your raw Markdown text stays canonical, byte for byte.

Built with Rust and [Dioxus](https://dioxuslabs.com/) Desktop on the
OS-native WebView.

## Why bekoedit

Most visual Markdown editors convert your text into a rich-text model and
serialize it back, normalizing your formatting along the way: list markers
change, fences shrink, blank lines move. bekoedit refuses to do that.

- **Raw Markdown is the document.** Every visual surface is a disposable
  projection of the text.
- **Edits are minimal patches.** Changing a heading rewrites that heading's
  bytes — nothing else. Your `*` lists stay `*`, your `~~~~` fences stay
  `~~~~`, your CRLF stays CRLF.
- **Unsafe regions become Raw Markdown Islands.** Front matter, HTML,
  tables, nested lists, and anything ambiguous render as clearly labeled
  raw-text regions instead of being lossily "understood".
- **Rust owns the source.** The WebView UI sends semantic intent; UTF-8
  byte ranges, file operations, and persistence never leave Rust.

## Features (MVP)

- Workspace explorer over a folder of Markdown files (create, rename,
  delete to trash), with recent workspaces
- Three modes over one canonical text: **Text** (raw source), **Form**
  (visual block editing), **Preview** (sanitized rendering; document HTML
  is escaped, scripts never run)
- Debounced autosave with atomic writes, crash-recovery snapshots, and
  external-change conflict resolution (keep mine / reload / save copy) —
  neither version is ever lost silently
- GUI internationalization (English and Japanese included)

## Repository structure

```
crates/
  bekoedit-markdown     parsing index, block identity, source patches,
                        form projection, raw islands, preview rendering
  bekoedit-fs           workspace scoping, file tree, safe file ops,
                        atomic save, recovery snapshots, recents
  bekoedit-core         document sessions, save lifecycle, conflicts,
                        application state store
  bekoedit-ui-contract  versioned command/event payloads for the
                        WebView boundary
  bekoedit-app          Dioxus Desktop shell (binary: `bekoedit`)
docs/                   mdBook-compatible documentation (docs/src)
rfcs/                   design RFCs (see rfcs/README.md)
ARCHITECTURE.md         architectural invariants (normative summary)
```

## Building

Rust 1.85+ (edition 2024). Headless crates build everywhere:

```sh
cargo test            # markdown, fs, core, ui-contract (default members)
```

The desktop app additionally needs the platform WebView. On Linux:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev
cargo run -p bekoedit-app
```

Windows uses WebView2 (preinstalled on Windows 11); macOS uses WKWebView.

## Development

- `cargo fmt --all` and `cargo clippy --workspace` must pass (CI enforces)
- Tests live in `tests.rs` submodules next to the code under test
- Source files target ≤300 effective lines
- Design changes go through the RFC process: see
  `rfcs/done/000-rfc-lifecycle-policy.md`

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
