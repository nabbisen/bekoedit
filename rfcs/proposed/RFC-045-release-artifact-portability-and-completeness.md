# RFC-045: Release Artifact Portability and Completeness

**Project:** bekoedit
**Status:** Proposed — approved for implementation by the project owner
2026-08-17. Slice 1 (ship the platform scripts) merged as `a9eb427`; slice 2
(the cross-distribution check) merged as `f977fc7`. Remains in `proposed/`
until slice 3 lands. §10 Q1 and Q2 are now answered — on slice 2's evidence
plus a verified local trial (§5.1) — and Q3 remains an owner action.
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
Linux artifact this project has shipped carries it.

*Refined by slice 2 (§10 Q1):* the `libxdo.so.3` requirement is as old as the
first prebuilt binary, but the **breakage** is younger — Arch moved to
`libxdo.so.4` on 2026-03-18, so v0.9.0 (2026-06-07) onward was already
unlaunchable there. Distributions still on `xdotool` 3.x are unaffected so far.

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

### 5.1 Remove the dependency (upstream) · the verified fix

bekoedit uses neither native menus nor a tray icon, and the switch to drop the
dependency **already exists upstream**:

- `muda` gates it: `libxdo = ["dep:libxdo"]`, on by default.
- `tray-icon` exposes the same gate (`default = ["libxdo"]`,
  `libxdo = ["muda/libxdo"]`) and already takes `muda` itself with
  `default-features = false`.

Only `dioxus-desktop` takes both dependencies with their defaults, and its own
feature set offers no way to opt out. Cargo features are additive, so **bekoedit
cannot turn this off from its own manifest** — adding a direct `muda` dependency
with `default-features = false` subtracts nothing from what another crate
enables.

**Measured, not assumed (2026-08-24).** A local trial patched `dioxus-desktop`
0.7.9 through `[patch.crates-io]` with three manifest lines —
`default-features = false` on `tray-icon`, and `default-features = false` plus
`features = ["gtk"]` on `muda` — and rebuilt bekoedit 0.15.0 unchanged:

| | shipped 0.15.0 artifact | patched build |
|---|---|---|
| `libxdo-sys` in the graph | present | **absent** — `cargo tree -i` matches no package |
| `DT_NEEDED` entries | 19 | 18 |
| set difference | — | `libxdo.so.3` removed, **nothing added** |
| `check-linux-portability.sh`, Arch-family host | exit 1, `UNRESOLVED: libxdo.so.3` | **exit 0** |
| `--headless-smoke`, same host | exit 127, loader failure before `main()` | **PASSED**, exit 0 |

No bekoedit source changed. The two `dioxus-desktop` warnings the patched build
emits are `#[cfg(all(feature = "devtools", debug_assertions))]`-gated dead code
in any release build — pre-existing upstream, surfaced only because `[patch]`
turns a registry crate into a path crate.

**What still blocks it:** the change must land in `dioxus-desktop`, and the
opt-out must be reachable from the `dioxus` facade, since bekoedit depends on
`dioxus` with the `desktop` feature rather than on `dioxus-desktop` directly.
That is one pull request in one repository (§10 Q2) — but it is still someone
else's release cycle, so this RFC does not block on it (§4).

### 5.2 Bundle `libxdo` beside the binary · the fallback, and worse than it looked

Ship `libxdo.so.3` in the archive and set an `$ORIGIN`-relative `RPATH` so the
loader finds it.

Still cheap in bytes: ~68 KB against a 4 MB artifact. And slice 2 confirmed it
would be *sufficient* — with the documented dependencies installed,
`libxdo.so.3` is the only unresolved library on Arch, and nothing is unresolved
on Fedora.

**The cost side is what moved.** Slice 2 also established what the split
actually is (§10 Q1): `xdotool` has a 4.x series, Arch adopted it on
2026-03-18, and Fedora and Ubuntu have not. Bundling therefore means shipping —
and owning the security updates for — a **superseded** build of a library
bekoedit never calls, indefinitely, so that a dead code path can satisfy the
loader.

Two of the three caveats are now checked rather than assumed:

- `libxdo`'s own closure (`libX11`, `libXtst`, `libXinerama`, `libxkbcommon`,
  `libxcb`, `libXext`, `libXau`) resolves on **both** distributions slice 2
  tests. Retired.
- Bundling means owning security updates — still true, and heavier now that the
  bundled SONAME is the superseded one.
- Wayland-only systems without XWayland remain **untested**. Neither container
  image has a display server, so slice 2 cannot speak to them.

Keep this as the fallback if §5.1 stalls upstream. Do not start here.

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

**Now disproportionate.** §5.1 removes the single offending library with three
lines of someone else's manifest. Reaching for a whole-closure bundle to solve
the same problem would be a large permanent cost against a small temporary one.
This stays on the list only for a future portability break that §5.1 cannot
address.

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

| # | Part | Status |
|---|---|---|
| 1 | Part 2 — ship the platform scripts | **Implemented** — merged `a9eb427` |
| 2 | Part 3 — the cross-distribution check | **Implemented** — merged `f977fc7` |
| 3 | Part 1 — Linux portability | Open — reshaped by §10 Q1 |

Deliberately in that order, and it worked as intended: the two cheap slices made
the expensive one measurable.

**Slice 2's gate is slice 3's acceptance test.** The Arch container carries
`--expect-missing libxdo.so.3`, which both permits and *requires* that library
to be absent. Whatever fix lands must make it resolve there and delete the
exemption in the same change, or the check fails. That coupling is deliberate:
the fix cannot land without retiring its own exemption.

## 10. Open questions

### Q1 — how do we fix Linux portability? · **answered: §5.1, with §5.2 as fallback**

The question as originally posed was "§5.2 or §5.3 — bundle `libxdo`, or ship an
AppImage?", with the honest caveat that the evidence was one machine. Slice 2
widened it, and the answer changed because *what the problem is* changed.

**This is not a distribution quirk. It is a migration in progress.**

| Distribution | `xdotool` | Provides | Observed |
|---|---|---|---|
| Arch (`extra/x86_64`) | 4.20260303.1, updated 2026-03-18 | `libxdo.so.4` | `/usr/lib/libxdo.so.4`; `libxdo.so.3` not found |
| Fedora 44 (current stable) | 3.20211022.1 | `libxdo.so.3` | `/lib64/libxdo.so.3`; zero unresolved |
| Fedora rawhide (`.fc45`) | 3.20211022.1-11 | `libxdo.so.3` | not yet migrated |
| Ubuntu (`ubuntu-latest`, the builder) | — | `libxdo.so.3` | the artifact's own `NEEDED` entry is the evidence |

Arch is not peculiar. Arch is **first**. The affected set is "distributions that
have adopted xdotool 4.x" — currently Arch-family, and it only grows.

Three consequences:

1. **§5.1 is the fix**, and it is verified rather than hoped for. It removes the
   dependency instead of picking a side of the split.
2. **§5.2 is the fallback**, not "the pragmatic answer". Bundling pins the side
   upstream has already left.
3. **Both distributions stay in slice 2's matrix**, even though Fedora passes
   today. When Ubuntu adopts xdotool 4.x the builder begins emitting binaries
   that need `libxdo.so.4` and the breakage inverts — Arch goes green, Fedora
   goes red. A single-distribution matrix would sail straight through that. Two
   distributions on opposite sides of a live SONAME migration is the
   instrument, not redundancy.

**Slice 3 is reshaped accordingly:** pursue §5.1 upstream (Q2) and hold §5.2 in
reserve. Either way, slice 2's Arch container is the acceptance test.

### Q2 — should §5.1 be raised with Dioxus? · **answered: yes, as a tested pull request**

Not an issue carrying a hypothesis. The change is three manifest lines, the
result is measured (§5.1), and both direct dependencies already expose the
switch deliberately — `muda` and `tray-icon` each ship a `libxdo` feature so
that downstreams can decline it. `dioxus-desktop` also already uses this exact
pattern one dependency away, for `rfd`:

```toml
rfd = { version = "0.17.2", default-features = false, features = ["xdg-portal"] }
```

**Draft:** `.git-exclude/governance/2026-08-24-dioxus-libxdo-upstream-request.md`

Shape it to be mergeable rather than merely correct: add a **default-on**
`libxdo` feature to `dioxus-desktop` forwarding to `muda/libxdo` and
`tray-icon/libxdo`, take both dependencies with `default-features = false`, and
plumb a matching pass-through through the `dioxus` facade. Default-on means no
existing user sees a behaviour change; downstreams that do not want an X11
input-simulation library linked into their binary can opt out.

### Q3 — does the release page need a Linux caveat? · **open, owner's call**

Unchanged, and now time-boxed by Q1. Users on Arch-family distributions still
download a binary that cannot start, and will until slice 3 lands.
`docs/src/distribution.md` and `README.md` say so; the release page does not.
`cargo install bekoedit` is the working route there and is unaffected by any of
this.
