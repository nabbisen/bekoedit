# RFC-045 handoff — slice 3b: both AUR packages, published from CI

**Governing RFC:** [RFC-045](../../accepted/RFC-045-release-artifact-portability-and-completeness.md) §5.5, §5.6
**Slice:** 3b. **Do not start before slice 3a is merged and its manual proof
run is green.**
**Baseline:** `main` with slice 3a merged.
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

At each tag, after the GitHub release is published, CI renders, proves and
pushes both AUR packages:

- [`bekoedit`](https://aur.archlinux.org/packages/bekoedit): builds from the
  tag's source tarball, from `packaging/linux/PKGBUILD`;
- [`bekoedit-bin`](https://aur.archlinux.org/packages/bekoedit-bin):
  repackages slice 3a's Arch archive.

Both AUR repositories hold the owner's placeholder today: `pkgver=0.0.0`, and
a `package()` that only prints "Placeholder". **The owner decided on full
automation (level 2)**, with the safeguards in RFC-045 §5.6. This slice builds
those safeguards. The owner's one-time setup (a separate CI account on the
AUR, its key, the `aur` environment) is not needed for this slice's work or
its dry run, only for the first real push.

## 2. Required · **[Binding]**

### 2.1 The `bekoedit-bin` template

A second PKGBUILD template in the repository. **Its path is [Advisory],** for
example `packaging/linux/bin/PKGBUILD`.

- `pkgname=bekoedit-bin`, `provides=('bekoedit')`, `conflicts=('bekoedit')`,
  and `pkgver`, `pkgrel=1`, `pkgdesc`, `arch`, `url` and `license` as in
  `packaging/linux/PKGBUILD`.
- **`source`:** the Arch archive from the release,
  `https://github.com/nabbisen/bekoedit/releases/download/$pkgver/bekoedit-$pkgver-x86_64-unknown-linux-gnu-arch.tar.gz`,
  and the `.desktop` file and icon **from the same tag**. Give each a local
  name that includes `$pkgname-$pkgver`, so a cached file from another version
  is never reused.
- **`depends`:** exactly what the binary needs on Arch, `xdotool` included,
  with the same comment as the source template: it is there only because
  Dioxus 0.7 links `libxdo`. `namcap` decides the list; report what it said.
- **`package()`** installs the same files to the same paths as the source
  package's `package()`: the binary, the `.desktop` file, the icon, `LICENSE`
  and `NOTICE`.
- `sha256sums=('SKIP' ...)` in the template, as the source template does; the
  rendered copy has the real ones.
- **`scripts/check-packaging-versions.sh` covers it,** like the source
  template: `pkgver` equal to the workspace version, and `pkgrel=1`. Correct
  that script's opening comment, which says no release-time automation
  publishes either package.

### 2.2 Rendering

A script, for example `scripts/aur-render.py`, that takes a version, a
template, and the files the sources point to, and writes a `PKGBUILD` ready
for the AUR:

- `pkgver` is the version; `pkgrel=1`.
- **Every `SKIP` is replaced** by the sha256 of the matching file, in source
  order. The script refuses if the counts differ, if any `SKIP` remains, or if
  the template's `pkgver` is not the version.
- **Template-only comments are removed.** The source template says it is a
  template and must not be used with `makepkg -si`. That is false in the AUR
  copy. Mark such comments in the template so the renderer can drop them
  (mechanism [Advisory]), and make sure no comment in either rendered file
  is false there.
- **`.SRCINFO` comes from `makepkg --printsrcinfo`** on the rendered file, as
  a non-root user. Never written by hand.
- **Unit tests** for each refusal above, and for one full render of each
  template, compared with an expected file.

### 2.3 Proving before pushing

A reusable workflow, for example `.github/workflows/aur.yml`, with
`workflow_call` (from `release.yml`) and `workflow_dispatch` (the dry run).
In a **clean** Arch container, pinned by digest, with a non-root build user:

*Learned from slice 3a (task 068):* in a `container:` job, the workspace is the
runner's mount, owned by another user than the container's root. Git then
treats it as no repository unless it is in the global `safe.directory`, which
the checkout sets only under a temporary home. Any git command on the
workspace in this container needs that same trust step. A clone made inside
the container, such as the AUR repository, does not.

1. Render both packages and their `.SRCINFO`.
2. **`makepkg` each one,** with dependencies, from a clean build directory.
   - **At a tag:** nothing is placed in advance. `makepkg` downloads the
     tag's source tarball and the **published** Arch archive, and verifies
     them against the rendered sums. That is the check that the sums match
     what users will download.
   - **In a dry run:** the source package uses the latest existing tag's
     tarball. The binary package needs an Arch archive, and none is published
     before slice 3a's first release. So the dry run runs slice 3a's build and
     places that archive in the build directory under its source name, where
     `makepkg` uses it **and still checks its sum.** Say in the request how
     you did it.
3. **`namcap`** on each `PKGBUILD` and each built package. An error fails the
   job; warnings are printed, and you report them.
4. **Install each package** with `pacman -U`, one at a time, removing the
   first before the second, since they conflict. For each:
   - `readelf -d /usr/bin/bekoedit` lists `libxdo.so.4` and not `.so.3`;
   - slice 2's `check-linux-portability.sh` on `/usr/bin/bekoedit`, **with no
     exemption**, passes;
   - `env -u DISPLAY -u WAYLAND_DISPLAY timeout 10 /usr/bin/bekoedit --headless-smoke`
     exits 0;
   - the `.desktop` file and the icon are at their paths.
5. **Upload the two rendered `PKGBUILD` and `.SRCINFO` pairs** as one
   artifact. The push job pushes exactly these files, nothing re-rendered.

### 2.4 Pushing

**A job in `release.yml` only,** for example `aur-push`. It is never in the
reusable workflow, so no manual run can reach the key.

- **`needs`** the proving job, which `needs` `publish`. `publish-crates` does
  not depend on it, and it does not depend on `publish-crates`.
- **`environment: aur`.** The key is the environment secret
  `AUR_SSH_PRIVATE_KEY`. The commit identity is the environment variables
  `AUR_GIT_NAME` and `AUR_GIT_EMAIL`, not hard-coded. The owner restricts the
  environment to tags.
- **Runs no project build and no third-party action.** `actions/checkout`
  and `actions/download-artifact`, pinned by commit as elsewhere, are allowed.
  No `cargo`, no `npm`, nothing from the repository but the push script and
  the host-key file.
- **The AUR host key is pinned in the repository,** for example
  `packaging/linux/aur-known-hosts`, used with `StrictHostKeyChecking=yes`
  and `IdentitiesOnly=yes`. Take the keys from `ssh-keyscan
  aur.archlinux.org` and **check each fingerprint against the ones published
  on the AUR's home page,** <https://aur.archlinux.org>. Quote both in the
  request. I check them again.
- **The key** is written with mode 600 under `$RUNNER_TEMP` and removed in a
  step that runs whatever happened (`if: always()`). It is never printed and
  never passed on a command line.

**The push script,** for example `scripts/aur-push.sh PACKAGE_DIR REMOTE`,
testable without the AUR. For each package:

1. Clone the AUR repository.
2. **Compare versions** by Arch's ordering (`vercmp`, or an equivalent you
   justify for bare SemVer plus an integer `pkgrel`), using the
   `pkgver-pkgrel` in the AUR's current `.SRCINFO`. The placeholder `0.0.0-1`
   counts as older.
   - **Newer:** copy in `PKGBUILD` and `.SRCINFO`, commit as
     `<pkgname> <pkgver>-<pkgrel>`, and push to `master`.
   - **Equal, and the files are identical:** "already up to date", success.
   - **Equal but different, or older:** fail, naming both versions. This is
     what stops a re-run of an old tag's workflow from pushing an older
     version, or overwriting a packaging fix the owner made by hand.
3. **Never `--force`.** If the AUR moved during the run, the push fails as a
   normal non-fast-forward.
4. **Any other files** in the AUR repository are left alone, and listed in the
   log.

Both packages are attempted even if the first fails. The job fails if either
did.

### 2.5 Documentation

- **A maintainer page,** for example `docs/src/aur-publishing.md`, in
  `SUMMARY.md`: what CI does at a tag, what it proves, how to re-run a failed
  push, how to replace the key, and the manual procedure as the fallback.
  **Not** `docs/src/manual-release-checklist.md`, which you must not edit.
- **No user-facing AUR mention** in `README.md`, `docs/src/distribution.md`
  or the release-page note. I add those once the first push is live, so that
  they only offer what exists.
- **No CHANGELOG entry:** nothing changes for users until a package is live.

## 3. Prohibited · **[Binding]**

- Any push to the AUR from your machine, or by anything in this slice except
  `aur-push` at a real tag. The dry run pushes nothing and holds no key.
- `--force`, or deleting files from an AUR repository.
- The push job in a workflow that `workflow_dispatch` can start.
- A third-party action in the push job.
- A change to slice 3a's build, the Ubuntu archive, or any exemption.
- Vendoring or `[patch]`ing any upstream crate.
- Running anything that initialises GTK, tao or a WebView on this machine.
  `--headless-smoke` runs in CI only.

## 4. Evidence and review request · **[Binding]**

- **The renderer's tests,** and both rendered files for the current version,
  in full, with their `.SRCINFO`.
- **The push script's tests, against local bare repositories:**
  - the first push over a `0.0.0-1` placeholder;
  - a re-run with identical files: up to date, no new commit;
  - an older version: refused;
  - the same version with different files: refused;
  - the remote moved during the run: a non-fast-forward failure, no force;
  - an extra file in the remote: left alone and listed.
- **The AUR host-key fingerprints,** from `ssh-keyscan` and from the AUR's
  home page, side by side.
- **Mutations, one each, every one caught by a named test:** the version
  comparison removed; `--force` added; the identical-file check removed; a
  `SKIP` left in a rendered file.
- **What you expect after the merge:** a dry run of the AUR workflow on
  `main`, green, with both packages built, checked, installed and started in
  its log. Then, at the next tag, after the owner's setup, the first real
  push.
- **Every local gate,** Rust tests with `CI` unset: `scripts/check-eloc.sh`,
  fmt, clippy 1.88.0 `-D warnings`, the workspace tests, the JS tests,
  `check-changelog`, `check-rfcs`, `check-packaging-versions`,
  `test-release-notes`, `test-release-artifacts`, the task 063 check and its
  self-test, `git diff --exit-code -- Cargo.lock`, and
  `git diff --stat origin/main...HEAD | tail -1`.

Review request: `.git-exclude/review-request/<date>-rfc-045-slice-3b-aur-publish.md`.
Give the full tip hash and its base.
