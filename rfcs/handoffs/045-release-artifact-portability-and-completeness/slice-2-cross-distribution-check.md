# RFC-045 handoff — slice 2: the cross-distribution check

**Governing RFC:** [RFC-045](../../accepted/RFC-045-release-artifact-portability-and-completeness.md) §7
**Slice:** 2 of 3 — independent of slice 3; it is what makes slice 3 measurable
**Baseline:** `main` after slice 1 merges (`a9eb427`). Slice 1 touches
`release.yml` and `ci.yml`; if it has not merged, **stop and report** rather
than branching from an earlier commit.
**Status:** inherited from RFC-045 (Accepted — approved for implementation 2026-08-17)
**Date:** 2026-08-17

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a mechanism you may replace with reasoning, without asking.

## 1. Purpose

Both defects in RFC-045 survived for the same reason: **no release artifact has
ever been executed anywhere but the machine that built it.** Slice 1 fixed one
symptom. This slice closes the gap that hid both.

The question to answer is narrow and cheap: *would this binary start on a
machine that is not the builder?* Not "does it work" — `ldd`, not a launch. No
display, no GUI, no synthetic input.

An Arch-based container would have caught the `libxdo.so.3` defect on 2026-06-07,
when v0.9.0 shipped the first prebuilt binary — ten weeks before the owner found
it by hand on 2026-08-17.

## 2. Shape — three pieces, and the split matters · **[Binding]**

**2.1 An inspection script that never installs anything.**
`scripts/check-linux-portability.sh <binary>` (name **[Advisory]**). Runs `ldd`,
partitions the unresolved libraries, exits non-zero on a violation. Pure
inspection: no package manager, no network, no write, no fix — the same rule
`run-linux.sh` already follows.

This is binding for a specific reason. The script must be safe to run on the
owner's own machine, which is Arch-family and is RFC-045 §8's reference system.
A script that can `pacman -S` is not safe to hand someone; and the check is only
worth having if it is reproducible outside CI, which this project has already
been burned on.

**2.2 A container harness that does the installing.** Pulls a pinned image,
installs the runtime dependencies, mounts the binary and the script, invokes
2.1 inside the container. This is the only piece allowed to install anything,
and it must do so **only inside `docker run`, never on the host.** If Docker is
unavailable it must fail, not fall back to a host-local check — a gate that
quietly degrades into a weaker gate is worse than one that stops.

**2.3 Two call sites**, sharing 2.1 and 2.2:

| When | Against what | Catches |
|---|---|---|
| Every pull request (`ci.yml`) | the freshly built `target/release/bekoedit` | a portability break at the moment it is introduced |
| Tag, in `release.yml`'s `publish` job | the Linux member extracted from the real archive | anything the packaging path itself changes |

RFC-045 §7 asked only for the second. I am requiring both, because slice 1
taught us the cost of a release-path gate: it cannot be exercised until a tag,
so its first real run is also its first test. The pull-request call site makes
this slice verifiable before it ships, and it is the one that will actually
catch the next break — a new `NEEDED` entry arrives with a dependency bump, in a
pull request, not at release time.

In `publish`, place it **after** `Verify complete release artifact set` and
**before** `Publish verified release`. A portability failure must block the
release.

## 3. Semantics — what passes, what fails · **[Binding]**

The script takes zero or more `--expect-missing NAME` arguments (spelling
**[Advisory]**). For each library `ldd` reports as unresolved:

- **not in `--expect-missing`** → fail, naming it.
- **in `--expect-missing`** → permitted.

And the anti-rot half, which is the part that keeps this from becoming
decoration:

- **an `--expect-missing` library that actually resolves** → fail, naming it as
  a stale expectation.

So `--expect-missing libxdo.so.3` means *permitted to be absent, and required to
be absent.* On the Arch container today that is a true statement. When slice 3
lands and `libxdo.so.3` resolves there, the run fails until the argument is
removed — which is exactly the coupling I want: slice 3 cannot land its fix
without deleting its own exemption.

The two failure texts must be **distinguishable**. "Unresolved library not
permitted" and "expected-missing library resolved" are opposite problems and a
reader at 2am should not have to work out which one happened.

Do not make the exemption a constant inside the script. It is per-run and
per-distribution; it belongs at the call site, next to a comment naming RFC-045
slice 3 as the thing that deletes it.

**Infrastructure failures must not read as portability defects.** If the image
pull or the package install fails, say so in those words and fail the step. A
distribution's mirror being down is not a finding about bekoedit, and it must
not be reported as one — nor silently skipped.

## 4. Which distributions · **[Binding on the property, Advisory on the choice]**

**At least two, at least one Arch-family.** Arch is binding because it is the
confirmed-failing family and the owner's own environment. The second is yours to
pick — Fedora is the obvious candidate, being a third packaging family.

Two, not one, because §10 Q1 says in as many words that the evidence for
bundling `libxdo` is *one machine*. Widening that is a stated purpose of this
slice, not a nicety.

**Pin the images by digest.** This repository pins every GitHub Action by commit
SHA; a container image tag is the same kind of moving target and gets the same
treatment.

## 5. The dependency list · **[Binding]**

A bare `archlinux:latest` has none of the WebView stack, so a naive `ldd` in it
reports dozens of missing libraries and says nothing. The container must first
install what a user of that distribution would have installed.

`docs/src/distribution.md` names the requirement in prose —
`libwebkit2gtk-4.1`, plus the transitive `libxdo` — but gives an install command
only for Debian/Ubuntu. **Establishing the per-distribution package list is part
of this slice**, and it must live somewhere durable and readable: in the harness
with a comment, not spread through workflow YAML.

If the documented requirements turn out to be incomplete — a library bekoedit
needs that the docs never mention — that is a finding, and a valuable one. Report
it; do not quietly add the package and move on.

## 6. Non-change scope · **[Binding]**

- **The portability defect itself.** Slice 3. This slice measures it; it must not
  fix it, bundle anything, or produce an AppImage.
- `run-linux.sh`'s user-facing output. It ships inside archives now; leave it be.
  Reusing its `ldd` parsing internally is **[Advisory]** — your call.
- Target triples, the build matrix, version numbers, `CHANGELOG.md`, `ROADMAP.md`.
- Any Rust code.
- Documentation promising that Arch-family distributions work. **They do not
  yet.** Do not write anything that implies otherwise; the doc update belongs
  with slice 3, when it becomes true.
- `docs/src/manual-release-checklist.md` — the owner is editing it.

## 7. Required tests · **[Binding]**

RFC-045 §8 is explicit and I am not softening it: **a gate that cannot be shown
to fail is not a gate**, and that applies with force to one whose entire purpose
is catching what CI missed.

1. **The check run against the real published 0.15.0 Linux artifact, with no
   `--expect-missing`, failing and naming `libxdo.so.3`.** Download it from the
   release page — it is the exact object under test, and the defect in it is
   real. This is the demonstration RFC-045 §8 requires.
2. The same artifact with `--expect-missing libxdo.so.3` — passes.
3. **Stale-expectation detection**: `--expect-missing` a library that does
   resolve in the container; the run must fail naming it as stale, not as
   missing.
4. An unresolved library that is *not* exempted fails — synthesise it however is
   cheapest (a `--expect-missing` you deliberately omit is enough).
5. The pull-request call site demonstrated green on a real CI run.

Prove each non-vacuous per standing practice — break it, watch it fail naming
the case, restore, confirm no residue. Per the slice-1 re-review: prefer patching
a **copy** in a scratch directory over backup-and-restore, so "no residue" is a
property of the method rather than something you verify afterwards.

## 8. Required evidence

Beyond the usual changed-file list and CI run:

- **The §10 Q1 input, which is the main reason this slice exists.** For each
  distribution tested, with documented dependencies installed: how many
  libraries `ldd` reports, and the complete list of those that do not resolve.
  If `libxdo.so.3` is the only one on both, say so plainly — that is the finding
  that decides bundle-versus-AppImage.
- **The §5.2 caveat, checked rather than assumed.** RFC-045 §5.2 says
  `libxdo.so.3` has its own dependencies (`libX11`, `libXtst` and similar) and
  that "near-universal needs checking rather than assuming." In each container,
  with the documented dependencies installed, report whether those resolve. It is
  nearly free here and it retires an open caveat.
- The per-distribution package list you established, and where each name came
  from.
- Wall-clock cost added to a pull-request run. If it is heavy enough to be worth
  gating on `Cargo.lock`/`Cargo.toml` changes, propose that — **[Advisory]**,
  with your measurement.

## 9. Prohibited shortcuts · **[Binding]**

- No package installation on the host. Containers only.
- No falling back to a host-local check when Docker is unavailable.
- No fixing the portability defect.
- No launching the binary — `ldd` only. There is no display, and RFC-042 §10's
  rule about the owner's live session is not suspended by a container being
  involved.
- No `--force` push.

## 10. Also in scope — a one-line carry-over · **[Binding]**

From the slice-1 re-review §4.1, which the dev team found and reported:
`scripts/test-release-artifacts.sh` treats an **empty** first argument as the
current directory, because its `[ "$#" -ne 1 ]` guard counts arguments and
`pathlib.Path("")` is `Path(".")`. Rehearsing the CI step locally without
`$RUNNER_TEMP` exported writes a scratch directory into the repository root.

Add the missing guard (`-z` alongside the arity check). No CI impact —
`$RUNNER_TEMP` is always set on a runner — but it lands here because this slice
is already in that tooling.

## 11. Acceptance criteria · **[Binding]**

1. The inspection script runs anywhere, installs nothing, and is usable by hand
   against an arbitrary binary path.
2. The harness installs only inside a container and fails rather than degrading
   when Docker is absent.
3. Both call sites wired: every pull request, and `publish` before the release is
   created.
4. At least two distributions, at least one Arch-family, images pinned by digest.
5. `--expect-missing` semantics per §3, including stale detection, with
   distinguishable failure text.
6. The five §7 cases covered and proven non-vacuous, including the failing run
   against the real 0.15.0 artifact.
7. The §8 evidence present, in particular the per-distribution unresolved-library
   lists that feed §10 Q1.
8. §10's guard added.
9. CI green; no Rust, no version, no packaging-layout change.

## 12. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL. Merging,
merge mechanism, tags, and releases require explicit instruction. If `main` has
moved, report the topology and stop.

Unlike slice 1, **most of this slice is exercisable by CI**, which is why §2.3
requires the pull-request call site. The one part that is not is the extraction
of the archive inside `publish`, since that job runs only on a tag. Say plainly
in the review request which of the two call sites CI actually ran.

Commit scopes: `ci:` for the workflow and harness, `chore:` for §10 if it is
separate. Split as it splits cleanly.

## 13. Review-request format

`.git-exclude/review-request/<date>-rfc-045-slice-2-cross-distribution-check.md`,
workflow policy §9.2 sections. **Lead with the §7 case 1 failure** — the real
0.15.0 artifact, checked and rejected, naming `libxdo.so.3`. That single result
is the whole argument for this slice, and it is the one this project should have
been able to produce since 2026-06-07, when v0.9.0 published the first prebuilt
binary.
