# Contributing

See [CONTRIBUTING.md](../../.github/CONTRIBUTING.md) in the repository root for the
full developer guide covering prerequisites, build instructions, test
requirements, code quality gates, commit conventions, and the RFC process.

## Quick reference

```sh
cargo fmt --all && cargo clippy --workspace -- -D warnings
cargo test --workspace
./target/debug/bekoedit --headless-smoke
bash scripts/check-rfcs.sh
```

All four must be green before a pull request is merged.

## Source-preservation invariants

Every change to `bekoedit-markdown` must preserve the property that applying
a `SourcePatch` to the canonical text and rebuilding the `MarkdownIndex`
produces a document where:

1. Only the target byte range changed.
2. All whitespace and marker trivia outside that range is identical.
3. The new block structure matches what the semantic edit intended.

Tests for these properties live in `crates/bekoedit-markdown/src/tests/`.
