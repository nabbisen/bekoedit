# Introduction

bekoedit is a desktop Markdown editor with one defining promise: **your
raw Markdown text is the document**, and editing it visually never
rewrites what you didn't touch.

You can edit in three modes over the same text:

- **Text** — the raw source.
- **Form** — headings, paragraphs, lists, tasks, and code blocks as real
  form controls. Regions that can't be edited safely (front matter, HTML,
  tables, nested lists…) appear as clearly labeled **Raw Markdown
  Islands** you edit verbatim.
- **Preview** — read-only sanitized rendering.

Switching modes never changes the document. Edits apply as minimal
patches: change a heading and only the heading's bytes change. Your list
marker style, fence characters, line endings, and blank lines survive.

bekoedit is built in Rust with Dioxus Desktop on the OS-native WebView,
and is licensed under Apache-2.0.
