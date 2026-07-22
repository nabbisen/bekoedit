# Release Evidence Log

Use this page as a template for each release-candidate evidence record. Copy the
sections into the release issue, handoff, or tag sign-off note, then replace
each placeholder with observed output, workflow links, artifact names, and the
maintainer decision.

Do not mark an item complete unless the command output, workflow result, or
artifact inspection was observed for the release candidate being signed off.

## Third-party action pin policy

Every third-party GitHub Action must use a reviewed full commit SHA in tracked
workflow `uses:` entries. Re-evaluate every pin during each release prep,
immediately after an upstream security advisory, deprecation, runtime/runner
incompatibility, or maintenance notice, and at least every 90 days when no
release triggers a review.

For each candidate, record the intended upstream release tag and verify that
its full executable commit is reachable from that tag. Dereference annotated
tags and record the peeled commit rather than the tag-object SHA. Review the
release notes, exact commit's `action.yml` runtime and inputs/outputs, required
permissions, runner compatibility, and relevant advisories or maintenance
notices.

A changed pin requires maintainer and architecture approval, affected local
and workflow/static gates, and fresh exact-commit CI. Keep a human-readable
release label in the workflow comment beside the SHA and record the mapping,
review date, and evidence in candidate release records.

An intentionally non-current release is an exception. Record the newer
release, the incompatibility or risk requiring the older selection, supported
runtime/runner assumptions, an owner, an event-driven re-evaluation trigger,
and an absolute review date no more than 90 days away. Never retain an older
pin silently.

---

## Candidate identity

| Field | Value |
|-------|-------|
| Candidate version | `X.Y.Z` |
| Commit SHA | `________` |
| Release branch | `________` |
| Evidence date | `YYYY-MM-DD` |
| Maintainer | `________` |

## Local gates

Record the command, environment, and observed result.

MSRV terminology is package-specific. Cargo declares Rust 1.88 for
`bekoedit`, `bekoedit-core`, and `bekoedit-markdown`. Exact Rust 1.85 CI
continuously tests `bekoedit-fs` without adding a manifest constraint in the
0.13.1 patch. `bekoedit-ui-contract` has no independent manifest MSRV. Exact
Rust 1.88.0 is the whole-workspace release compiler; this does not imply that
every newer compiler is separately certified.

| Gate | Evidence |
|------|----------|
| `cargo +1.88.0 fmt --all -- --check` | `________` |
| `cargo metadata --locked --format-version 1` | `________` |
| Parsed package MSRV split (three at 1.88; fs/UI contract absent) | `________` |
| `cargo +1.85.0 test -p bekoedit-fs --locked` | `________` |
| `cargo +1.88.0 clippy --workspace --all-targets --locked -- -D warnings` | `________` |
| `cargo +1.88.0 test --workspace --locked` | `________` |
| `cargo +1.88.0 audit` | `________` |
| `bash scripts/check-rfcs.sh` | `________` |
| `mdbook build docs` | `________` |
| `cargo +1.88.0 build -p bekoedit --release --locked --target <target>` | `________` |
| Target-qualified binary architecture inspection | `________` |
| Target-qualified release binary `--headless-smoke` | `________` |
| `bash scripts/test-release-artifacts.sh <temporary-directory>` | `________` |
| `git diff --exit-code -- Cargo.lock` | `________` |

## CI gates

| Gate | Evidence |
|------|----------|
| Latest CI workflow run | `________` |
| Format and clippy job | `________` |
| Workspace tests on Linux, macOS, Windows | `________` |
| Rust 1.85 filesystem support job | `________` |
| Rust 1.88 native GUI builds on Linux, macOS, Windows | `________` |
| Security audit job | `________` |
| Build and smoke job | `________` |
| Audit warning decision | `accept / defer / deny`: `________` |

## Release workflow artifacts

Record the tag workflow run and every generated artifact.

For every builder, also record `RUNNER_OS`, `RUNNER_ARCH`, full `rustc -vV`
output, requested and installed target, target-qualified Cargo output path,
platform binary-inspection output, archive name, and the immediately
pre-upload Cargo.lock cleanliness result. These values must agree with the
fixed workflow matrix.

The publisher must invoke `scripts/check-release-artifacts.sh` on the merged
download directory. After verification, pass exactly those six explicit
archive/sidecar paths to the publication action without an intervening step or
wildcard.

| Artifact | Root layout inspected | Checksum verified | Notes |
|----------|-----------------------|-------------------|-------|
| Linux `.tar.gz` | `yes / no` | `yes / no` | `________` |
| macOS `.tar.gz` | `yes / no` | `yes / no` | `________` |
| Windows `.zip` | `yes / no` | `yes / no` | `________` |

Expected root layout after extraction:

```text
bekoedit[.exe]
README.md
LICENSE
NOTICE
CHANGELOG.md
```

Expected checksum sidecars:

```text
bekoedit-<version>-<target>.tar.gz.sha256
bekoedit-<version>-<target>.zip.sha256
```

## Manual release walkthrough

Manual testing belongs here so it stays separate from automated gate evidence.
Use the [Manual Release Checklist](manual-release-checklist.md) for the
detailed reusable checklist. Copy that checklist into the per-release evidence
record, fill it with observed results, and keep release-instance evidence
separate from this template.

| Area | Evidence |
|------|----------|
| Launch release binary | `________` |
| Open workspace and document | `________` |
| Edit in Text Mode | `________` |
| Edit supported blocks in Form Mode | `________` |
| Confirm Raw Markdown Island behavior | `________` |
| Save and reload without source loss | `________` |
| External modification conflict handling | `________` |
| IME composition check | `________` |
| Keyboard-only primary workflow | `________` |

## Accepted risks

List only risks that are intentionally accepted for this release candidate.

| Risk | Decision | Follow-up |
|------|----------|-----------|
| Unsigned binaries | accepted | Signing remains future work. |
| `cargo audit` warnings | `________` | `________` |
| Manual testing gaps | `________` | `________` |

## Maintainer sign-off

```text
[ ] All local gates above were observed for this commit.
[ ] Latest CI run passed for this commit.
[ ] Release workflow artifacts and checksum sidecars were inspected.
[ ] Manual release walkthrough was completed or explicitly deferred.
[ ] Accepted risks are recorded above.

Signed: __________
Date: YYYY-MM-DD
```
