# Architecture

bekoedit is structured as a Cargo workspace of five crates. Each layer has a
single responsibility; the layers form a strict dependency graph — no crate
depends on one above it.

```
┌─────────────────────────────────────────────────────────────┐
│  bekoedit-app   Dioxus Desktop shell, UI components,        │
│                  WebView bridge, keyboard shortcuts          │
├─────────────────────────────────────────────────────────────┤
│  bekoedit-core  AppState, document sessions, save lifecycle, │
│                  conflict detection, local history           │
├─────────────────────────────────────────────────────────────┤
│  bekoedit-fs    Workspace scoping, file tree, atomic writes, │
│                  recovery, search, backlinks, Git status,    │
│                  workspace templates                         │
├─────────────────────────────────────────────────────────────┤
│  bekoedit-markdown  MarkdownIndex, block identity,          │
│                      source patches, form projection,        │
│                      preview rendering, section operations   │
├─────────────────────────────────────────────────────────────┤
│  bekoedit-ui-contract  Typed command/event payloads         │
│                         (versioned; shared across boundary)  │
└─────────────────────────────────────────────────────────────┘
```

## The source-preservation principle

Raw Markdown is the **canonical source**. The UI never holds an authoritative
copy of the document — it only projects the current canonical text into
visual surfaces (Form Mode cell grids, Preview HTML, the CodeMirror buffer).

All mutations flow through `bekoedit_markdown::resolve_form_edit`, which
returns a `SourcePatch`: a (byte-range, replacement-string) pair that is the
**minimum edit** required to apply a semantic change. The patch is applied to
the canonical text in memory; the index is rebuilt from scratch; the UI
re-renders from the new index.

## The WebView boundary (RFC-002)

The Dioxus Desktop shell embeds a platform-native WebView for the CodeMirror
text editor and the preview surface. Communication crosses the boundary via a
typed relay:

- **Rust → JS**: `document::eval(js)` sends raw JavaScript.
- **JS → Rust**: `dioxus.send(json_string)` enqueues a message. Rust receives
  it via `relay.recv().await` in a `use_coroutine`.
- The relay JS is installed once per component mount via `bridge::relay_js`.

All messages are validated against `bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION`
so mismatches surface as explicit errors rather than silent data corruption.

## Editing modes

| Mode | Surface | Mutation path |
|------|---------|--------------|
| Text | CodeMirror 6 (WebView) | JS sends full-text diff → Rust applies as `EditText` |
| Form | Dioxus RSX components | `FormBlockEdit` → `resolve_form_edit` → `SourcePatch` |
| Preview | HTML in WebView | Read-only; no mutations |
| Split | Text + Preview side-by-side | Same as Text |

## File-system safety

All writes go through `bekoedit_fs::atomic_write`, which writes to a `.tmp`
file alongside the target and renames it into place. This ensures the original
is never partially overwritten.

Workspace operations are scoped: all paths are validated against the workspace
root to prevent directory-traversal writes.
