# bekoedit

[![License](https://img.shields.io/github/license/nabbisen/bekoedit)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/bekoedit?label=rust)](https://crates.io/crates/bekoedit)
[![Rust Documentation](https://docs.rs/bekoedit/badge.svg?version=latest)](https://docs.rs/bekoedit)
[![Dependency Status](https://deps.rs/crate/bekoedit/latest/status.svg)](https://deps.rs/crate/bekoedit)
[![CI](https://github.com/nabbisen/bekoedit/actions/workflows/ci.yml/badge.svg)](https://github.com/nabbisen/bekoedit/actions/workflows/ci.yml)

**A source-preserving Markdown editor. Edit visually — your raw Markdown
stays canonical, byte for byte.**

Built with Rust + [Dioxus](https://dioxuslabs.com/) Desktop on the OS-native
WebView. No Electron. No cloud. No data loss.

---

## Overview

bekoedit (*from Japanese 逆 abekobe — two sides*) gives you two complementary
ways to work on the same document:

- **Text Mode** — raw Markdown in CodeMirror 6 with syntax highlighting
- **Form Mode** — visual block editor that patches only the changed bytes
- **Preview Mode** — rendered read-only view
- **Split Mode** — text and preview side by side

The invariant that holds across all modes:

> **The raw Markdown file is canonical. Every visual surface is a projection.
> Every edit is a minimal source patch.**

---

## Why not other editors

| | bekoedit | Raw-text editor | WYSIWYG editor |
|--|---------|-----------------|----------------|
| Source preserved exactly | ✅ | ✅ | ❌ rewrites on save |
| Visual editing | ✅ Form Mode | ❌ | ✅ |
| Local files, no account | ✅ | ✅ | varies |
| Lightweight (no Electron) | ✅ | ✅ | usually ❌ |
| CJK / multibyte safe | ✅ tested | ✅ | varies |

---

## Quick Start

Download the latest release for your platform from
[Releases](https://github.com/nabbisen/bekoedit/releases).

**macOS** — binary is unsigned; run once to clear the quarantine flag:
```sh
chmod +x scripts/run-macos.sh && ./scripts/run-macos.sh ./bekoedit
./bekoedit
```

**Windows** — unblock in PowerShell once:
```powershell
.\scripts\run-windows.ps1
.\bekoedit.exe
```

**Linux** — no extra step:
```sh
chmod +x bekoedit && ./bekoedit
```

**Build from source** (requires Rust stable + Node.js >= 24):
```sh
git clone https://github.com/nabbisen/bekoedit
cd bekoedit && cargo run -p bekoedit
```

---

## Features

**Source-preserving patches** — Form Mode edits touch only the target byte
range. CRLF endings, tilde fences, non-1 ordered lists, reference links,
front matter, HTML blocks, and GFM tables survive round-trips unchanged.
Unsupported structures become **Raw Markdown Islands** — never silently
normalized.

**Form Mode** — paragraphs, headings, bullet/ordered/task lists, blockquotes,
fenced code blocks, images, inline links, simple GFM tables. Bold / Italic /
Code / Link toolbar. Unsupported structures shown as editable raw islands.

**Text Mode** — CodeMirror 6 with syntax highlighting, CJK/IME
composition-safe (sends to Rust only after `compositionend`), find-in-file,
undo/redo.

**Workspace & files** — local folder tree, create/rename/delete (trash by
default), `.git`/`node_modules`/`target` ignored, Git status badges (M/A/D/?).

**Safe saves** — atomic writes (temp-file + rename), autosave debounce,
external-modification detection, conflict resolution, crash-recovery snapshots,
local document history (last 50 saves per document).

**Navigation** — outline panel with section move, backlinks, full-text search,
section-reorder shortcuts.

**Accessibility** — keyboard-only workflows, `role="tree"` / `role="treeitem"`,
ARIA live regions for save status, EN + JA interface.

**Export & templates** — one-click HTML export, workspace templates, math block
display (LaTeX source; KaTeX-ready).

---

## Design Notes

Five Rust crates with strict dependency ordering — no crate depends on one
above it. The WebView boundary is versioned: JS sends compact JSON intent;
Rust validates, resolves byte ranges, applies patches, persists.

**Test coverage:** 149 tests covering adversarial documents (CRLF + emoji +
tilde fences + non-1 ordered lists + reference links + front matter + HTML +
tables), UTF-8 boundary safety, stale-revision rejection, raw island
preservation, atomic save, conflict detection, dirty-document protection, and
conflict-safe section/history mutations.

---

## More Detail

[Getting Started](docs/src/getting-started.md) ·
[Editing Modes](docs/src/editing-modes.md) ·
[Source Preservation](docs/src/source-preservation.md) ·
[Architecture](docs/src/architecture.md) ·
[Contributing](.github/CONTRIBUTING.md) ·
[Distribution](docs/src/distribution.md)

---

## License

Apache-2.0 — see [LICENSE](LICENSE). Copyright © nabbisen.
