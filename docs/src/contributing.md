# Contributing

See [CONTRIBUTING.md](../../.github/CONTRIBUTING.md) in the repository root for the
full developer guide covering prerequisites, build instructions, test
requirements, code quality gates, commit conventions, and the RFC process.

## Quick reference

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked                 # default-members: core, fs, markdown, ui-contract
cargo test -p bekoedit --locked     # the app crate (needs WebView deps installed)
cargo build -p bekoedit --release --locked
./target/release/bekoedit --headless-smoke
```

These mirror the gates in `.github/workflows/ci.yml`. CI additionally enforces
a per-file ELOC limit, runs a WebView lifecycle regression and an eval-script
parse-check (both need a real display, so they aren't reproducible in a plain
headless shell), and runs `cargo audit`. `bash scripts/check-rfcs.sh` verifies
RFC status fields and numbering; it isn't wired into CI, but is worth running
before opening a pull request that touches `rfcs/`.

## Source-preservation invariants

Every change to `bekoedit-markdown` must preserve the property that applying
a `SourcePatch` to the canonical text and rebuilding the `MarkdownIndex`
produces a document where:

1. Only the target byte range changed.
2. All whitespace and marker trivia outside that range is identical.
3. The new block structure matches what the semantic edit intended.

Tests for these properties live in `crates/bekoedit-markdown/src/tests/`.
