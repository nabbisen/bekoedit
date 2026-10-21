# RFC-045 handoff — slice 3a: the interim Arch-built archive

**Governing RFC:** [RFC-045](../../accepted/RFC-045-release-artifact-portability-and-completeness.md) §5.5, §6, §7, §10 Q3
**Slice:** 3a. Slice 3b (publishing both AUR packages from CI, §5.6) follows
it and depends on it. Slice 3 itself (Dioxus 0.8) stays deferred, and retires
this one.
**Baseline:** `main` at `e4c597f` or later.
**Status:** inherited from RFC-045 (Accepted 2026-08-17, amended 2026-10-21)
**Date:** 2026-10-21
**Workflow:** the shared folder, detached HEAD, no branch. **No push until the
review's written merge instruction.** Every task's local gates include
`scripts/check-eloc.sh` and the task 063 check.

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

## 1. Purpose

Each release also builds the Linux binary in an Arch Linux container. That
binary links `libxdo.so.4`, so it starts on Arch-family systems, where the
Ubuntu-built one cannot (RFC-045 §2.1). It is published as two more release
files:

```
bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz
bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz.sha256
```

Slice 3b's `bekoedit-bin` AUR package repackages it. **Nothing about the
Ubuntu archive changes**, and its Arch exemption stays, still required.

## 2. Required · **[Binding]**

### 2.1 The Arch build job

- **A reusable workflow,** for example `.github/workflows/arch-build.yml`,
  with `workflow_call` (for `release.yml`) and `workflow_dispatch` (for proof
  runs by hand). **No `push` or `pull_request` trigger:** the owner accepted
  10–15 CI minutes per release, not per push. It never publishes anything.
- **The version:** at a tag, `github.ref_name`, validated as bare SemVer like
  the other jobs. On a manual run, the workspace version from `Cargo.toml`.
- **The container:** `archlinux:base`, **pinned by digest**, as slice 2's
  harness pins its images. It starts with a full `pacman -Syu`: Arch is
  rolling, and a partial upgrade is unsupported.
- **One list of Arch build packages,** in one script, for example
  `scripts/install-arch-build-deps.sh`, as task 063 did for apt. It installs
  at least the compiler toolchain, `pkgconf`, `git`, `webkit2gtk-4.1`, `gtk3`,
  `xdotool` and `openssl`, plus extra packages given as arguments. Every job
  that builds `bekoedit` in an Arch container calls it.
- **The same toolchain, lockfile and tests as the Ubuntu build:**
  `rustc 1.88.0`, verified with `rustc -vV` (host
  `x86_64-unknown-linux-gnu`), then `cargo +1.88.0 metadata --locked`,
  `cargo +1.88.0 test --locked --target x86_64-unknown-linux-gnu`, the
  release build with `--locked`, and `git diff --exit-code -- Cargo.lock`.
- **Assert the point of the job:** `readelf -d` on the binary lists
  `libxdo.so.4` and does **not** list `libxdo.so.3`. Fail otherwise, quoting
  the `NEEDED` lines.
- **Start it once, headless,** in the build container, exactly as `ci.yml`
  does for the Ubuntu binary:
  `env -u DISPLAY -u WAYLAND_DISPLAY timeout 10 ./bekoedit --headless-smoke`,
  exit 0.
- **The archive:** the same six members as the Ubuntu archive (`bekoedit`,
  `README.md`, `LICENSE`, `NOTICE`, `CHANGELOG.md`, `run-linux.sh`), built the
  same way, with a `.sha256` in the same format. Uploaded as an artifact named
  so that the `publish` job's existing `pattern: release-*` picks it up.

### 2.2 `release.yml`

- **`publish` needs the Arch job as well as `build`.** If the Arch build
  fails, the release is not published, like any other target. Say in the
  request how a re-run behaves.
- **The release gets eight files:** the two new lines in the publish step's
  `files`.
- `publish-crates` is unchanged.

### 2.3 The artifact check

`scripts/check-release-artifacts.sh`: a fourth target, `-arch`, with the same
members as the Ubuntu archive. **Exactly eight files**, and the exact-member
check per archive. **Do not relax either to a subset** (RFC-045 §6).

`scripts/test-release-artifacts.sh`, new cases, each failing with its own
message:

- the Arch archive missing;
- its `.sha256` missing, or wrong;
- a ninth, unexpected file;
- the Arch archive with a member missing, or an extra member.

### 2.4 The portability check

- **The Ubuntu archive's check is unchanged:** both distributions, Arch's
  `libxdo.so.3` exemption still required.
- **The Arch archive:** the publish job extracts its binary and runs slice 2's
  harness on it **in the Arch container only, with no exemption.** Every
  library must resolve. Not on Fedora: it has no `libxdo.so.4`.
- **The mechanism is [Advisory],** for example an option on
  `check-linux-portability-containers.sh` that selects one distribution and
  ignores the exemptions file. **The exemptions file keeps its one meaning:**
  the Ubuntu binary's expected gaps. `release-notes.py` reads it, and slice 3
  empties it.
- **Show it can fail:** the Arch-only, no-exemption check run on the
  **Ubuntu** binary fails, naming `libxdo.so.3`.

### 2.5 The release page note

`scripts/release-notes.py`: while the Arch exemption exists, the "Known issue
(Linux)" note now names the Arch archive. For example:

```markdown
### Known issue (Linux)
The default Linux archive does not start on Arch Linux and Arch-based
distributions, which ship `libxdo.so.4` instead of `libxdo.so.3`. On those,
use `bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz` instead, or
`cargo install bekoedit`.
```

With the real version, not `<version>`. **No mention of the AUR in this
slice:** it is added once CI has published there (slice 3b). Extend
`test-release-notes` to cover the text.

### 2.6 `run-linux.sh`

It already explains a missing `libxdo.so.3`. Add the opposite: a missing
`libxdo.so.4` means this is the Arch archive on a distribution that ships
`.so.3`, so use the default Linux archive. Say both cases in the comment at
the top.

### 2.7 The task 063 check

`scripts/check-linux-build-deps.py` must keep passing, and keep meaning
something:

- **A job running in an Arch container** that builds `bekoedit` must run the
  Arch build script before its first build step. It must not be required to
  run the apt script, and must not pass by running it.
- **The apt rules are unchanged:** every other Linux job, and no raw
  `apt-get install` of the build packages.
- **A raw `pacman` install of the Arch build list in a workflow step** fails
  the check, as a raw apt install does. Slice 2's harness installs runtime
  packages inside `docker run`. Say how the check treats it.
- `scripts/test-check-linux-build-deps.py`: a case for each rule above,
  passing and failing.

### 2.8 Documentation and CHANGELOG

- `docs/src/distribution.md`: the Arch archive in the Linux section, for Arch
  and Arch-based distributions, with its checksum. The section that says an
  AUR package is planned stays as it is: I change it when a package is live.
- `README.md`: the same, briefly, where it lists the downloads.
- `docs/src/release-evidence.md`: eight files, and the Arch archive's root
  layout.
- `CHANGELOG.md`, `## [Unreleased]`, `### Added`, for example: "An Arch Linux
  build of the Linux binary,
  `bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz`, for Arch and
  Arch-based distributions, where the default Linux binary does not start."

## 3. Prohibited · **[Binding]**

- Any change to the Ubuntu, macOS or Windows builds or archives, or to the
  Ubuntu archive's exemption.
- A `push` or `pull_request` trigger for the Arch build.
- Vendoring or `[patch]`ing any upstream crate, or a `Cargo.toml` or
  `Cargo.lock` change.
- Weakening the exact-set checks, or any other gate.
- Anything that touches the AUR. That is slice 3b.
- Running anything that initialises GTK, tao or a WebView on this machine.
  `--headless-smoke` runs in CI only.

## 4. Evidence and review request · **[Binding]**

- **A local container proof** of the Arch build, if Docker is available here:
  the build and test in the pinned image, `readelf -d`'s `NEEDED` lines, the
  archive's members, and the harness's Arch-only run on it, verbatim. No
  `--headless-smoke` locally. If Docker is not available, say so: the proof
  run below then carries it alone.
- **The forced failure** from §2.4, verbatim.
- **The checker's tests and the task 063 self-test,** with their output.
- **`release-notes.py` output for a sample version,** verbatim.
- **Mutations, one each, every one caught by a named test or check:**
  - the artifact check back to six files;
  - the Arch archive's member set loosened to a subset;
  - the `libxdo.so.3` assertion in §2.1 removed (show what catches it, or say
    plainly that only the proof run would);
  - the Arch job calling the apt script instead of the Arch one.
- **What you expect after the merge:** a manual run of the Arch build workflow
  on `main` (I will ask the owner to start it, or you may, once merged), green,
  with the `readelf` and `--headless-smoke` lines in its log.
- **Every local gate,** Rust tests with `CI` unset: `scripts/check-eloc.sh`,
  fmt, clippy 1.88.0 `-D warnings`, the workspace tests, the JS tests,
  `check-changelog`, `check-rfcs`, `check-packaging-versions`,
  `test-release-notes`, `test-release-artifacts`, the task 063 check and its
  self-test, `git diff --exit-code -- Cargo.lock`, and
  `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-045-slice-3a-arch-archive.md`.
Give the full tip hash and its base.
