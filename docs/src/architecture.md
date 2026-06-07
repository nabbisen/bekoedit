# Architecture Overview

See `ARCHITECTURE.md` in the repository root for the normative
invariants. In brief:

```
bekoedit-app          Dioxus Desktop shell (thin; UI only)
bekoedit-ui-contract  versioned command/event payloads
bekoedit-core         sessions, store, save lifecycle, conflicts
bekoedit-markdown     index, identity, patches, form, islands, preview
bekoedit-fs           workspace, tree, safe ops, atomic write, recovery
```

The four headless crates are the workspace default-members and carry the
test suite; `cargo test` exercises them without any GUI dependency. The
UI sends *intent*; Rust validates and mutates. Projections flow back.

Key flows:

- **Form edit**: command (revision + block identity) → validate → resolve
  to minimal patch → apply → reparse → rebuild projections.
- **Save**: conflict check → atomic write → fingerprint update → recovery
  snapshot removal.
