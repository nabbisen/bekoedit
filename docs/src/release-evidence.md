# Release Evidence Log

Use this page as a template for each release-candidate evidence record. Copy the
sections into the release issue, handoff, or tag sign-off note, then replace
each placeholder with observed output, workflow links, artifact names, and the
maintainer decision.

Do not mark an item complete unless the command output, workflow result, or
artifact inspection was observed for the release candidate being signed off.

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

| Gate | Evidence |
|------|----------|
| `cargo fmt --all -- --check` | `________` |
| `cargo clippy --workspace --all-targets -- -D warnings` | `________` |
| `cargo test --workspace` | `________` |
| `cargo audit` | `________` |
| `bash scripts/check-rfcs.sh` | `________` |
| `mdbook build docs` | `________` |
| `cargo build -p bekoedit --release` | `________` |
| `./target/release/bekoedit --headless-smoke` | `________` |

## CI gates

| Gate | Evidence |
|------|----------|
| Latest CI workflow run | `________` |
| Format and clippy job | `________` |
| Workspace tests on Linux, macOS, Windows | `________` |
| Security audit job | `________` |
| Build and smoke job | `________` |
| Audit warning decision | `accept / defer / deny`: `________` |

## Release workflow artifacts

Record the tag workflow run and every generated artifact.

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
