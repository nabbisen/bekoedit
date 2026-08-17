# RFC-045 handoff — slice 1: ship the platform scripts

**Governing RFC:** [RFC-045](../../proposed/RFC-045-release-artifact-portability-and-completeness.md) §6
**Slice:** 1 of 3 — self-contained; does not depend on §10 Q1
**Baseline:** `main` at `8d3634b`
**Status:** inherited from RFC-045 (Proposed — approved for implementation 2026-08-17)
**Date:** 2026-08-17

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a mechanism you may replace with reasoning, without asking.

## 1. Purpose

`README.md` has told macOS users to run `scripts/run-macos.sh` since v0.8.0.
That script has never been in any archive. Same for `run-windows.ps1`.

The macOS case is the sharpest: `run-macos.sh` strips the quarantine attribute,
which is the *entire* first-run workflow for an unsigned binary under RFC-024. A
macOS user following the README hits "no such file or directory" before they can
get past Gatekeeper.

This slice ships the scripts so the documented instructions work.

## 2. Placement — archive root, not a `scripts/` subdirectory · **[Binding]**

Put each script at the archive root beside the binary:

```
bekoedit            run-linux.sh      (Linux tar)
bekoedit            run-macos.sh      (macOS tar)
bekoedit.exe        run-windows.ps1   (Windows zip)
README.md  LICENSE  NOTICE  CHANGELOG.md
```

Two reasons, and the second is the one that would otherwise bite:

**The archive is deliberately flat.** Five entries, no wrapper directory. A
single-file subdirectory per platform sits oddly against that.

**`check-release-artifacts.sh` rejects backslashes in member names**
(`validate_member_name`). PowerShell's `Compress-Archive` has historically
written path separators inconsistently for nested entries. Keeping every member
at the root means there is no separator to get wrong. If you place scripts in a
subdirectory instead, you own proving the Windows zip records forward slashes —
root placement avoids the question.

**One script per platform.** Do not ship a `.ps1` to Linux users or a `.sh` to
Windows users.

## 3. The layout is asserted in four places — they move together · **[Binding]**

| Location | Change needed |
|---|---|
| `.github/workflows/release.yml:129–133` | tar staging and member list |
| `.github/workflows/release.yml:144–147` | zip staging and member list |
| `scripts/check-release-artifacts.sh` | `TARGETS`/`DOCUMENTS` — the expected set now varies by target |
| `scripts/test-release-artifacts.sh` | fixtures for the checker's own tests |

`check-release-artifacts.sh` currently applies one `DOCUMENTS` tuple to every
target and compares with `actual != expected_members`. The expected set is now
per-target: `{binary} | DOCUMENTS | {script}`. Extending the `TARGETS` tuple with
the script name is the obvious shape; the mechanism is **[Advisory]**.

**Keep the comparison exact.** Do not relax it to a subset or a "contains"
check. That strictness is what turns an accidental extra or missing member into
a failed publish rather than a shipped surprise, and it is the only reason this
defect could not recur silently.

Also update the documented root layout in `docs/src/release-evidence.md`, which
currently lists the five-entry form.

## 4. Executable bit · **[Binding]**

`run-linux.sh` and `run-macos.sh` must arrive executable. A user told to run
`./run-macos.sh` who first has to work out `chmod` has been failed twice.

`tar` preserves mode; confirm it survives the staging copy — `cp` does by
default, but verify rather than assume, and state how you verified. Zip does not
carry a Unix mode usefully, which is fine: `.ps1` is invoked through PowerShell,
not executed directly.

## 5. Documentation · **[Binding]**

Update `README.md` and `docs/src/distribution.md` so the paths match what ships:
`./run-macos.sh`, `.\run-windows.ps1`, `./run-linux.sh` — no `scripts/` prefix.

**A wrinkle to handle deliberately, not trip over.** `README.md` ships *inside*
the archive and is also the repository's front page. Its Quick Start section
addresses someone who has just downloaded a release, so root-relative paths are
correct there. Someone working from a clone finds the scripts under `scripts/`,
and that is the build-from-source path lower down.

Do not try to make one instruction serve both. Quick Start speaks to archive
users; say so plainly enough that a later reader does not "fix" it back.

For Linux, task 011 currently gives an inline `ldd` command because the script
was unreachable. Now that it ships, point at the script. Keeping the inline
command as an alternative is **[Advisory]** — your call whether it earns its
space.

## 6. Non-change scope · **[Binding]**

- **The libxdo portability defect.** Slice 3, pending RFC-045 §10 Q1. This slice
  ships a script that *reports* the problem; it does not fix it.
- The cross-distribution CI check — slice 2.
- Target triples, build flags, the release matrix, version numbers,
  `CHANGELOG.md`, `ROADMAP.md`.
- Any Rust code.
- The scripts' own behaviour. Ship them as they are; if one is wrong, that is a
  separate finding.
- `docs/src/manual-release-checklist.md` — the owner is editing it.

## 7. Required tests · **[Binding]**

`test-release-artifacts.sh` drives `check-release-artifacts.sh` against
generated fixtures. Extend it so it covers:

1. A well-formed archive **with** the correct script for its platform — passes.
2. An archive **missing** the script — fails.
3. An archive with an **unexpected extra** member — still fails, proving the
   exact-set property survived the change.
4. An archive carrying the **wrong platform's** script — fails.

Case 3 is the one that matters most: it is the regression guard on the property
§3 tells you to preserve, and it is exactly the kind of check that quietly stops
working when the code around it is rewritten.

Prove each new case non-vacuous per standing practice — break it, watch it fail
naming the case, restore.

## 8. Acceptance criteria · **[Binding]**

1. Each archive contains its own platform's script, at the root.
2. `.sh` scripts arrive executable; verified and stated.
3. All four assertion sites updated; the member-set comparison remains exact.
4. `README.md` and `docs/src/distribution.md` give paths that work for someone
   holding only an extracted archive.
5. `docs/src/release-evidence.md`'s documented layout matches reality.
6. The four §7 cases covered and proven non-vacuous.
7. `bash scripts/test-release-artifacts.sh <tmp>` passes; `mdbook build docs`
   succeeds; CI green.
8. No Rust, no version, no release-matrix change.

## 9. Prohibited shortcuts · **[Binding]**

- No relaxing the exact member-set comparison.
- No shipping a platform's script to another platform.
- No touching the libxdo defect.
- No `--force` push.

## 10. Required evidence

- Changed-file list.
- The member list of each produced archive — run
  `scripts/test-release-artifacts.sh` and show the fixtures, or build one
  locally and `tar tzf` it.
- How you verified the executable bit survives.
- The four §7 cases with their non-vacuity evidence.
- CI run result.

## 11. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL. Merging,
merge mechanism, tags, and releases require explicit instruction. If `main` has
moved, report the topology and stop.

**Note that CI cannot fully exercise this.** The release workflow only runs on a
tag, so the real archive contents are not produced by a pull-request run. The
fixtures in `test-release-artifacts.sh` are the substitute, and they run in CI —
which is precisely why §7 case 3 matters. Say plainly in the review request that
no real archive was built by CI for this change.

Commit scope `ci:` for the workflow and checker changes, `docs:` for the
documentation. Two commits if that splits cleanly; one if it does not.

## 12. Review-request format

`.git-exclude/review-request/<date>-rfc-045-slice-1-ship-platform-scripts.md`,
workflow policy §9.2 sections. Lead with the produced archive member lists.
