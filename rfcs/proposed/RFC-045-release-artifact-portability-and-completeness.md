# RFC-045: Release Artifact Portability and Completeness

**Project:** bekoedit
**Status:** Proposed
**Track:** Distribution
**Priority:** High — the Linux artifact cannot start on a whole family of distributions
**Date:** 2026-08-17
**Related RFCs:** [RFC-024](../done/RFC-024-packaging-and-unsigned-distribution-ux.md), [RFC-025](../done/RFC-025-release-ci-smoke-tests-and-build-verification.md)
**Finding:** `.git-exclude/governance/2026-08-17-linux-artifact-libxdo-finding.md`

---

## 1. Summary

Make the release artifacts work for the people who download them. Two defects,
both long-standing, both found within minutes of the first manual walkthrough
ever run on a non-Ubuntu distribution:

1. The Linux binary **cannot launch** on distributions that ship
   `libxdo.so.4` rather than `.so.3`, and fails silently from a desktop
   launcher.
2. The platform helper scripts are **not in any archive**, so the documented
   first-run instructions for all three platforms reference files the user
   does not have.

## 2. Motivation

bekoedit has published prebuilt binaries since v0.9.0. Nobody had run one on a
distribution other than the Ubuntu family until 2026-08-17, when the owner
extracted the 0.15.0 tarball on CachyOS and it refused to start.

That is the whole motivation: **the artifacts have never been verified to work
on a machine other than the one that built them.** Both defects below follow
from the same gap, and neither is exotic.

### 2.1 The Linux binary is not portable

```
error while loading shared libraries: libxdo.so.3:
cannot open shared object file: No such file or directory
```

`readelf -d` confirms `libxdo.so.3` among the `NEEDED` entries. The chain is
`dioxus-desktop → muda + tray-icon → libxdo → libxdo-sys`. `muda` is Dioxus's
native-menu library and `tray-icon` its system tray — **bekoedit uses neither.**
Its menus are in-app Dioxus components. The dependency is entirely incidental.

The SONAME is fixed at build time by GitHub's `ubuntu-latest` runner. Arch-family
systems provide `libxdo.so.4` only. On the owner's machine, `ldd` resolves 144 of
145 libraries — `libwebkit2gtk-4.1`, `libsoup-3.0` and the rest are all present.
**`libxdo` is the single point of failure.**

The 0.13.1 artifact was downloaded and inspected: identical requirement. Every
Linux artifact this project has shipped has the defect.

Launched from a desktop environment the failure is silent — the loader gives up
before `main()`, so no bekoedit code runs and nothing can report it. That cannot
be fixed inside the binary.

`cargo install bekoedit` is unaffected; it compiles locally against whatever
`libxdo` is present.

### 2.2 The platform scripts are not shipped

`release.yml:131` packages exactly `bekoedit`, `README.md`, `LICENSE`, `NOTICE`,
`CHANGELOG.md`. There is no `scripts/` directory in any archive.

Yet `README.md` has instructed macOS users to run `scripts/run-macos.sh` since
the platform scripts landed in v0.8.0, and Windows users to run
`scripts/run-windows.ps1`. Neither file reaches them.

The macOS case is the worst of the three: `run-macos.sh` strips the quarantine
attribute, which is the *entire* first-run workflow for an unsigned binary
(RFC-024). A macOS user following the README hits "no such file or directory"
before they can clear Gatekeeper.

Task 011 corrected the Linux instructions to use an inline `ldd` command
instead. That was the right fix for one platform under a doc-only task; it is
not the right long-term answer for three.

## 3. Goals

- The Linux artifact starts on mainstream distributions, not only Ubuntu-family.
- Documented first-run instructions reference files the user actually has.
- The gap that hid both — no artifact ever executed off its build machine —
  gets a check, not just a fix.

## 4. Non-goals

- Code signing, on any platform. RFC-024's unsigned posture stands (DEC-009).
- Distribution packaging — no `.deb`, `.rpm`, AUR, Homebrew, or Flatpak. This
  is about the artifacts already published.
- Changing the supported target triples.
- Fixing `muda`/`tray-icon` upstream as a *prerequisite*. Worth pursuing (§5.1),
  but this RFC must not block on someone else's release cycle.

## 5. Part 1 — Linux portability

Four approaches, in the order I would try them.

### 5.1 Remove the dependency (upstream)

bekoedit uses neither native menus nor a tray icon. If `dioxus-desktop`
feature-gated `muda` and `tray-icon`, `libxdo` would disappear from `NEEDED`
entirely — no bundling, no size cost, less linked code.

Its current features (`tokio_runtime`, `transparent`, `devtools`,
`dioxus-signals`, `fullscreen`, `gnu`) offer no such gate. This is worth raising
with Dioxus regardless of what else we do, because it is the only option that
makes the problem cease to exist rather than be worked around.

**Not a plan on its own** — it depends on an upstream decision and release we do
not control.

### 5.2 Bundle `libxdo` beside the binary · likely the pragmatic answer

Ship `libxdo.so.3` in the archive and set an `$ORIGIN`-relative `RPATH` so the
loader finds it.

Cheap: the library is ~68 KB against a 4 MB artifact. And on the evidence we
have, it is *sufficient* — `libxdo` was the only unresolved library on a system
two distribution families away from the builder.

Caveats to establish before committing:

- `libxdo.so.3` has its own dependencies (`libX11`, `libXtst` and similar). They
  are near-universal on X11 systems, but "near-universal" needs checking rather
  than assuming.
- Wayland-only systems without XWayland may lack them entirely. The owner's
  machine runs Wayland *with* XWayland, so this is untested territory.
- Bundling a system library means owning its security updates. For a 68 KB
  input-simulation library that bekoedit never calls, that is a small but real
  obligation.

### 5.3 AppImage

The conventional answer to "a Linux binary that runs anywhere": bundle the whole
dependency closure into one self-contained file.

It would also cover any *future* portability break, not just this one. The cost
is that the WebView stack comes with it — `libwebkit2gtk-4.1` and its closure
are large, and the artifact would grow by a large multiple. **I have not
measured it and will not guess**; if this option is pursued, the first task is
to build one and report the size.

It also adds a second Linux artifact type, which touches the release matrix, the
layout checkers, and the evidence process.

### 5.4 Rejected

**Build on a `libxdo.so.4` distribution** — moves the breakage to Ubuntu users,
who are the majority.

**Advise a `.so.3 → .so.4` symlink** — a SONAME bump signals an ABI break. It
may work in practice for `xdo`'s small API; "may work in practice" is not
something this project should tell users to rely on, and if it half-works the
failure mode is a crash rather than a clean load error.

## 6. Part 2 — ship the platform scripts

Include `scripts/run-linux.sh`, `run-macos.sh`, and `run-windows.ps1` in their
respective archives, so the documented instructions work.

**The layout is asserted in four places and they must move together:**

| Location | What it asserts |
|---|---|
| `.github/workflows/release.yml:131–132` | tar members (Linux, macOS) |
| `.github/workflows/release.yml:146–147` | zip members (Windows) |
| `scripts/check-release-artifacts.sh:21` | `DOCUMENTS` tuple, plus an **exact** member-set comparison that rejects unexpected entries |
| `scripts/test-release-artifacts.sh:32–39` | fixtures for the checker's own tests |

Plus the documented root layout in `docs/src/release-evidence.md`, and the
instructions in `README.md` and `docs/src/distribution.md`.

The exact-set check in `check-release-artifacts.sh` is a feature here: adding a
member without updating it fails the publish job loudly rather than shipping a
surprise. Whatever lands must keep that property — **do not relax it to a subset
match.**

**Per-platform contents**, since shipping a `.ps1` to Linux users is noise:
Linux and macOS tarballs get their own `.sh`; the Windows zip gets the `.ps1`.

## 7. Part 3 — close the gap that hid this

Both defects survived because **no release artifact has ever been executed
anywhere but its build machine.**

The minimum worth adding: after the release workflow builds the Linux archive,
extract it in a container based on a **different** distribution and check that
its libraries resolve — `ldd`, not a full launch, since no display is available.
An Arch-based image would have caught this exact defect at build time.

This does not need a real GUI run, and should not attempt one. It needs to
answer "would this binary start on a machine that is not the builder?" — which
is precisely the question nobody was asking.

macOS and Windows have no equivalent cheap check and are out of scope here.

## 8. Testing

- Part 1: the chosen approach verified on a `libxdo.so.4` system — the owner's
  machine is the reference, and the check is that the extracted artifact
  launches.
- Part 2: `test-release-artifacts.sh` extended so its fixtures cover the new
  members, including negative cases for a *missing* script and an *unexpected*
  extra member. The exact-set property must be demonstrated still to hold.
- Part 3: the container check demonstrated failing against the current 0.15.0
  artifact and passing against whatever Part 1 produces. A gate that cannot be
  shown to fail is not a gate — standing practice in this project, and it
  applies with force to one whose whole purpose is catching what CI missed.

## 9. Slices

1. **Part 2** — ship the scripts. Self-contained, unblocks three platforms'
   documentation, no dependency on Part 1's outcome.
2. **Part 3** — the cross-distribution check. Independent, and it gives Part 1
   its acceptance test.
3. **Part 1** — portability, once §5.2's caveats are established. Slice 2 makes
   this verifiable rather than hopeful.

Deliberately in that order: the two cheap slices make the expensive one
measurable.

## 10. Open questions

1. **§5.2 or §5.3 — bundle `libxdo`, or ship an AppImage?** I lean 5.2 on
   evidence and cost, but the evidence is one machine. Slice 2 would widen it
   before committing.
2. **Should §5.1 be raised with Dioxus now?** It costs an issue and might remove
   the problem at the root for everyone. I would file it regardless of which
   option we implement.
3. **Does 0.15.0's release page need a Linux caveat in the meantime?** Users on
   affected distributions currently download a binary that cannot start. The
   documentation now says so; the release page does not. Owner's call.
