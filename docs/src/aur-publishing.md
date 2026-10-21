# Publishing to the AUR

Both AUR packages are published from CI at each release tag
([RFC-045](https://github.com/nabbisen/bekoedit/blob/main/rfcs/accepted/RFC-045-release-artifact-portability-and-completeness.md)
§5.6):

- [`bekoedit`](https://aur.archlinux.org/packages/bekoedit) builds from the tag's
  source tarball, from `packaging/linux/PKGBUILD`;
- [`bekoedit-bin`](https://aur.archlinux.org/packages/bekoedit-bin) repackages
  the release's Arch archive
  (`bekoedit-<version>-x86_64-unknown-linux-gnu-arch.tar.gz`), from
  `packaging/linux/bin/PKGBUILD`. It `provides` and `conflicts` with `bekoedit`.

The Microsoft Store package stays manual.

## What CI does at a tag

After the GitHub release is published (`release.yml`, job `publish`):

1. **`aur-prove`** (the reusable workflow `aur.yml`) renders both packages in a
   clean Arch container, pinned by digest, then builds, checks, installs and
   starts each one. Nothing is pushed, and no key is available to it.
2. **`aur-push`** pushes exactly the two proven `PKGBUILD` and `.SRCINFO` pairs.
   It runs only if `aur-prove` passed. It is independent of `publish-crates`:
   neither waits for the other, and a failure of one does not stop the other.

The GitHub release and crates.io do not depend on the AUR. If the AUR push fails,
they are already done.

## What is proved before anything is pushed

`scripts/aur-prove.sh`, for each package:

- **Render.** `scripts/aur-render.py` fills `pkgver`, `pkgrel=1` and every
  checksum, in source order, from the files the template's `source` points to,
  and drops the comments that describe the template (between `# >>> template-only`
  and `# <<< template-only`). It refuses a template whose `pkgver` is not the tag,
  a different number of files and checksums, or a `SKIP` that would remain.
  `.SRCINFO` comes from `makepkg --printsrcinfo`, never by hand.
- **`makepkg`** from a clean build directory, as an ordinary user. At a tag,
  nothing is placed in advance: `makepkg` downloads the tag's tarball and the
  **published** Arch archive and checks them against the rendered checksums. That
  is the check that the checksums are what users will download.
- **`namcap`** on the `PKGBUILD` and every built package, the debug one included.
  An error fails the job; warnings are printed. One error is excused, by its exact
  shape: the debug package's build-id symlinks point into the main package, which
  `namcap` cannot see (`scripts/aur-namcap-errors.py`).
- **Install and start.** Exactly the package under proof is installed with
  `pacman -U` (`scripts/aur-pick-package.py` picks it from the built files by its
  exact package name), and removed before the next, since they conflict. Arch's
  default `makepkg.conf` also builds a `<pkgname>-debug` package; it is checked by
  `namcap` but never installed, because it would outlive the removal and conflict
  with the next package's own debug file. The installed
  `/usr/bin/bekoedit` must list `libxdo.so.4` and not `libxdo.so.3`, pass
  `scripts/check-linux-portability.sh` with no exemption, and exit 0 from
  `--headless-smoke`. The `.desktop` file and the icon must be in place.

## What the push refuses

`scripts/aur-push.sh` clones the AUR repository and compares `pkgver-pkgrel` with
the one in the AUR's current `.SRCINFO` (a repository with none counts as older,
as does the owner's `0.0.0-1` placeholder):

| New compared with the AUR's | Result |
|---|---|
| newer | commits `<pkgname> <pkgver>-<pkgrel>` and pushes to `master` |
| equal, files identical | "already up to date", success, no commit |
| equal, files different | fails: it would overwrite a fix made by hand |
| older | fails: a re-run of an old tag must not push an older version |

It first checks that the package is the repository's: the `.SRCINFO`'s `pkgbase`,
the package directory's name and the repository's name must be the same, so a
mix-up stops before any commit. It never uses `--force`; if the AUR moved during the run, the push fails as an
ordinary non-fast-forward. It never deletes: any other file in the AUR repository
is left alone and listed in the log. It compares only bare `N.N.N` versions and
integer `pkgrel`s, which gives Arch's `vercmp` result for them
(`scripts/test-aur-push.sh` checks the two against each other); anything else,
including an `epoch`, is refused rather than guessed at.

## The key and the environment

The push job uses the GitHub environment `aur`, which holds the SSH key
`AUR_SSH_PRIVATE_KEY` (a secret) and the commit identity `AUR_GIT_NAME` and
`AUR_GIT_EMAIL` (variables). The key belongs to a separate AUR account that is a
co-maintainer of these two packages only, so it can never act as the owner. The
environment is restricted to tag refs.

The job runs no project build and no third-party action, and the key is written
with mode 600 under `$RUNNER_TEMP` and removed in a step that runs whatever
happened. The AUR's host keys are pinned in `packaging/linux/aur-known-hosts` and
checked strictly (`StrictHostKeyChecking=yes`, `IdentitiesOnly=yes`); their
fingerprints are the ones published on <https://aur.archlinux.org>.

**To replace the key:** create a new key pair, add its public half to the CI
account on the AUR, put the private half in the `aur` environment as
`AUR_SSH_PRIVATE_KEY`, and remove the old public key from the AUR account.

**If the AUR rotates its host keys,** the next push fails at the host-key check.
Take the new keys from `ssh-keyscan aur.archlinux.org`, check each fingerprint
against the AUR home page, and replace the lines in `packaging/linux/aur-known-hosts`
and the fingerprints in its header (a test compares them).

## Trying it without a release

`aur.yml` can be started by hand (**Actions**, **AUR packages**, **Run workflow**).
That is a dry run: it pushes nothing and holds no key. The source package uses the
workspace version's existing tag. The Arch archive for `bekoedit-bin` is built in
the same run, because none is published before the first release that carries one;
it is placed in the build directory under its source name, so `makepkg` uses it and
still checks its checksum.

## Re-running a failed push

Re-run the failed jobs of the release workflow run. `aur-push` is safe to repeat:
a package that is already published is "already up to date", and a re-run can never
push an older version.

## The manual procedure (the fallback)

If CI cannot push, take the proven files from the `aur-packages` artifact of the
release workflow run (or render them: `python3 scripts/aur-render.py VERSION
TEMPLATE OUTPUT_DIR FILE...`), and for each package clone
`ssh://aur@aur.archlinux.org/<pkgname>.git`, copy in `PKGBUILD` and `.SRCINFO`,
commit as `<pkgname> <pkgver>-<pkgrel>`, and push to `master`.
