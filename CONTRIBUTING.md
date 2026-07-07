# Contributing to bekoedit

Thank you for considering a contribution. This document covers the project
layout, development workflow, and the conventions used throughout the codebase.

## Prerequisites

| Tool | Minimum version | Purpose |
|------|-----------------|---------|
| Rust (stable) | 1.85 | All Rust crates |
| Node.js | 22 | Build the CodeMirror 6 bundle |
| Git | Any recent | Version control |

Install Rust via [rustup](https://rustup.rs). The workspace toolchain file
(`rust-toolchain.toml`) pins the exact compiler version automatically.

## Repository layout

```
crates/
  bekoedit-markdown   Parsing, block index, source patches, form projection
  bekoedit-fs         Workspace, file tree, search, history, templates, Git status
  bekoedit-core       AppState, document sessions, save lifecycle, conflicts
  bekoedit-ui-contract Typed command/event payloads (WebView boundary)
  bekoedit-app        Dioxus Desktop shell (binary: bekoedit)
docs/                 mdBook documentation
rfcs/                 done/ (implemented), proposed/ (decision or deferred)
scripts/              check-rfcs.sh (RFC integrity), release steps
benches/              Performance benchmarks (reparse.rs)
```

## Building

```sh
# First-time: install the Dioxus CLI (needed only to rebuild the JS bundle)
cargo install dioxus-cli --version 0.7

# Build and run in development mode
cargo run -p bekoedit-app

# Build release binary
cargo build --release -p bekoedit-app

# Rebuild the CodeMirror 6 bundle after editing js/src/editor.js
cd crates/bekoedit-app/js && npm install && npm run build
```

## Tests

```sh
# Full workspace test suite
cargo test --workspace

# Headless smoke test (CI equivalent)
cargo build -p bekoedit-app && ./target/debug/bekoedit --headless-smoke

# Performance benchmark (RFC-032)
cargo bench -p bekoedit-markdown -- --test
```

All three must pass before submitting a pull request.

## Code quality gates

Every pull request must pass:

```sh
cargo fmt --all                                    # formatting
cargo clippy --workspace -- -D warnings            # no new warnings
cargo test --workspace                             # all tests green
./target/debug/bekoedit --headless-smoke           # smoke test
bash scripts/check-rfcs.sh                        # RFC integrity
```

A file over **300 ELOC** (non-blank, non-comment lines) should be split.
The hard limit is **500 ELOC**; no file may exceed it.

## Source-preservation invariants

The project's core promise is that editing a document via Form Mode produces
source patches that are **minimal** and **reversible**. A patch must:

1. Touch only the byte range it claims to affect.
2. Preserve all whitespace, indentation, and marker style outside that range.
3. Round-trip through `MarkdownIndex::build` without changing the block structure.

Every `FormBlockEdit` variant has a corresponding test in
`crates/bekoedit-markdown/src/tests/form_tests/`.

## RFC process

New features require an RFC in `rfcs/proposed/` before implementation.
An RFC must include: motivation, design decisions, and implementation notes.
The `scripts/check-rfcs.sh` script verifies status fields and numbering.

Implemented RFCs move to `rfcs/done/` when the code ships.

## Commit messages

```
<scope>: <short present-tense description>

Optional longer explanation. Reference RFC numbers where applicable:
implements RFC-029, evaluates RFC-032.
```

Scopes: `markdown`, `fs`, `core`, `app`, `ui-contract`, `docs`, `ci`, `deps`.

## Releasing

Releases are tagged `vX.Y.Z`. The release checklist is in
`docs/src/distribution.md`. **v1.0.0 requires explicit maintainer sign-off**
on every item in `docs/src/mvp-acceptance.md`.
