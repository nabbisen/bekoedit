# Contributing and RFC Process

## Ground rules

- Rust edition 2024; `cargo fmt --all` and `cargo clippy --workspace`
  must pass (CI enforces both, plus tests on Linux/macOS/Windows).
- Tests live in `tests.rs` submodules next to the code under test and
  target the design specifications (RFC acceptance criteria), not the
  implementation.
- Keep source files at or under ~300 effective lines.
- License is Apache-2.0; contributions are accepted under the same terms.

## RFC process

Design decisions flow through RFCs in `rfcs/`, governed by
`rfcs/done/000-rfc-lifecycle-policy.md`:

- `rfcs/proposed/` — accepted direction, not (fully) implemented
- `rfcs/done/` — implemented; status notes name the shipping version
- `rfcs/archive/` — superseded or withdrawn

Each RFC states motivation, goals/non-goals, data model, and acceptance
criteria. Changes that alter an architectural invariant require updating
RFC-000 and `ARCHITECTURE.md` together.
