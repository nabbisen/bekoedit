# RFC-044: Shell Behaviour Regression Coverage

**Project:** bekoedit
**Status:** Proposed
**Track:** Verification infrastructure
**Priority:** High
**Date:** 2026-08-12
**Depends on:** [RFC-043](./RFC-043-reopen-last-workspace-on-launch.md) — required, see §6
**Related RFCs:** [RFC-041](../done/RFC-041-source-editor-lifecycle-and-synchronization-controller.md), [RFC-042](../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md), [RFC-025](../done/RFC-025-release-ci-smoke-tests-and-build-verification.md)

---

## 1. Summary

Give the shell's keyboard and focus contracts reproducible automated coverage,
by adding a **second, independent WebView smoke run** rather than extending the
existing RFC-041 regression.

## 2. Motivation

RFC-042 delivered five slices of keyboard and focus behaviour: tree navigation,
menu and tab contracts, focus authority arbitration, and accessibility metadata
across six surfaces. **None of it has automated behavioural coverage.**

The existing WebView regression runs a fixed seven-milestone sequence — Start
Screen → New → Text → edit → Preview. It never opens a workspace, a menu,
Settings, Recovery, or a conflict. Everything RFC-042 built was verified by
source inspection, guard tests asserting substrings, and a JavaScript parse
check.

That is not a hypothetical weakness. Slice 3 shipped two `document::eval`
templates that rendered to invalid JavaScript, making the entire slice's feature
inert. Every gate passed — formatter, linter, 101 tests, headless smoke, all
seven CI jobs. It was caught by a reviewer rendering the string and running it
through a parser by hand.

The current substitute is a manual release walkthrough. That substitute has a
defect this project has already been bitten by twice: it produces ticked
checkboxes that cannot be re-derived, cannot detect regression, and decay into
false guarantees. `mvp-acceptance.md` carried a ✅ whose cited evidence never
existed; the plain-language guard was documented as passing after it had been
deleted. A fifty-item manual walkthrough is the same artifact in a larger size.

**Reproducibility is the property being bought here.** A one-time observation of
correct behaviour and an automated check of correct behaviour are different
kinds of thing, and only the second survives contact with the next change.

## 3. Goals

- Automated, repeatable verification of the shell keyboard contracts RFC-042
  defined, running on every push.
- Preserve the existing RFC-041 regression **exactly** — no change to its
  sequence, milestones, or phase machine.
- Keep the gate trustworthy: deterministic waits, no sleeps, no flake.
- Minimise blind iteration, given the constraint in §7.

## 4. Non-goals

- Replacing the manual release walkthrough entirely. Some things stay human:
  IME composition, actual screen-reader announcement, visual rendering.
- Screen-reader certification. This verifies focus and DOM state, not what
  assistive technology says about it.
- Cross-OS coverage. §9 records why, and what it would take.
- A general-purpose UI test framework. This is one more smoke run, not a
  harness.

## 5. Core design decision — a second run, not a longer one

I have described this work in four prior handoffs as "extend the driver with a
phase." **That framing was wrong**, and the reason matters.

The existing run asserts an exact milestone sequence
(`validate_driver_result` compares against `EXPECTED_MILESTONES` with `.ne()`),
and its Launch phase requires the Start Screen and clicks `start-new`. Once
RFC-043 auto-opens a workspace, that precondition no longer holds. Extending the
sequence therefore means changing the milestone contract, the `SmokePhase` enum,
and the phase machine's transition table — all inside the one gate that
currently protects the RFC-041 lifecycle, which is the most defect-dense
subsystem in the codebase.

**Instead: a second run mode with its own profile, driver, phases, and milestone
list.** The RFC-041 regression stays byte-identical.

| | Existing run | New run |
|---|---|---|
| Entry | Start Screen → New | seeded workspace, auto-opened |
| Covers | bundle, mount, Text focus, Preview | tree, menus, tabs, focus authority |
| Milestones | unchanged | its own list |
| Failure | blocks, as today | blocks, once stable (§10) |

The two share the profile-isolation machinery and the poll-with-timeout
discipline, and nothing else.

## 6. Why RFC-043 is a hard dependency

The new run needs the application to reach the shell **without a file dialog.**
`rfd` uses `xdg-portal`, so a picker is a D-Bus request rendered by the session
portal — it escapes display isolation entirely and appears on the developer's
real desktop. No amount of Xvfb fixes that.

RFC-043's launch-time reopen is the dialog-free path. Two requirements fall out
of this RFC and belong on RFC-043's implementation:

1. Reopen must honour `AppPersistence::Isolated`, not only `PlatformDefault` —
   already binding in that handoff's §3.4.
2. The profile must be seedable. `SmokeProfile::create` requires the root **not
   to exist**, so CI cannot pre-populate it; the harness must create the
   workspace fixture and write the recents entry and settings itself, after
   creating the profile and before launching the shell.

## 7. The constraint that governs everything — no local execution

The dev environment has no `xvfb-run`, and driving the owner's live session is
prohibited (RFC-042 §10). So a WebView driver would be written, pushed, and
debugged **blind through CI**, in a blocking gate.

That is the exact objection on which I declined this work during RFC-042 slice 1,
and it has not changed. It must be resolved before implementation, not during.

Three ways, not mutually exclusive:

- **Install `xvfb-run` in the dev environment.** One package. Gives a real local
  loop for everything except the portal dialog, which this design avoids anyway.
- **Move driver logic into the bundled JavaScript**, where the existing
  `js/test/*.test.mjs` suite already runs in CI and locally with no display. The
  driver is currently `include_str!`'d rather than bundled; relocating the
  *decision* logic — which selector, which key, which assertion, what counts as
  done — leaves only a thin DOM-driving shell that must run in a real WebView.
  This is the structural fix and it shrinks the blind surface to near nothing.
- **Land it non-blocking first** (§10).

**Recommendation: the second, plus the first.** The second is worth doing on its
own merits — it makes the driver's logic unit-testable for the first time — and
it is what makes the rest of this RFC implementable responsibly.

## 8. Coverage, in priority order

Each item is a check the manual supplement currently asks a human to perform.

**A. Tree navigation** — Tab reaches the tree at exactly one stop; Down/Up move;
Right expands and enters; Left collapses and ascends; Home/End; a non-openable
row is reachable and not skipped; Enter opens a document and the editor takes
focus.

**B. Menus** — trigger Down focuses the first item and Up the last; in-menu
Up/Down wrap; Home/End; Escape closes and **restores** focus to the trigger; Tab
closes and does **not** restore.

**C. Mode tabs** — Left/Right move focus and **do not** change mode; Enter
activates. This one is worth naming separately: automatic activation would fire
one RFC-041 protected command per keystroke, and only an executed test can show
that it does not.

**D. Focus authority** — open a menu, move focus into the editor: the menu
closes and focus **stays** in the editor. This is slice 1's C3, a defect found
by reading code, whose fix has never been observed running.

**E. Settings and Recovery** — Settings entry focuses its heading and exit
restores to the app-menu trigger; a seeded recovery snapshot renders the
Recovery screen with focus on its heading.

**F. Conflict banner** — the harness modifies the open file from Rust mid-run;
the banner appears and focus **does not move**. Automatable because the harness
owns the filesystem, and worth automating because the wrong behaviour here
destroys user data.

A and B are the bulk of the untested surface and should land first. C–F are
follow-on slices under this RFC.

## 9. Cross-OS

The regression runs on Linux only. WebView2 and WKWebView are unverified, and
focus behaviour is the property most likely to differ between engines.

GitHub's `windows-latest` and `macos-latest` runners can run GUI processes, so
extending the matrix is technically possible. It is **out of scope here**:
tripling a gate's surface before it has proven stable on one platform is how
gates become flaky and then ignored. Revisit once §10's stability bar is met.

## 10. Stability before blocking

A blocking gate that flakes gets disabled, and then protects nothing.

**Land the new run non-blocking** (`continue-on-error`), and promote it to
blocking only after an agreed number of consecutive green runs on `main`.

This project has been here before: `ci.yml` carried a stale
`continue-on-error: true` on the smoke step long after the flag it guarded was
implemented, and it took an audit to notice. So the promotion must be scheduled
when the non-blocking period begins, with the criterion written down — not left
as an intention.

Determinism requirements, inherited from the existing harness: poll for a
condition with a timeout, never sleep; every wait names what it is waiting for;
a timeout reports which condition never became true.

## 11. Testing this test

- The new phase machine gets pure Rust tests, as the existing one has.
- Driver decision logic gets JavaScript unit tests in the existing npm suite
  (§7).
- Every assertion must be proven able to fail — break the behaviour, watch the
  check fail naming what broke, restore. This is standing practice and applies
  with extra force to a test whose whole purpose is catching what other tests
  cannot.

## 12. Acceptance criteria

1. The RFC-041 regression is byte-identical and still blocking.
2. A second run covers §8 A and B, executing real key events and asserting real
   focus.
3. It runs in CI on every push and pull request.
4. No sleeps; every wait is a condition with a timeout and a named failure.
5. Every assertion proven able to fail.
6. §7 resolved — the implementer has a local loop or the logic is unit-testable
   without a display.
7. A written promotion criterion from non-blocking to blocking, with a date.

## 13. Alternatives considered

**Extend the existing run.** Rejected — §5. It changes the milestone contract of
the gate protecting the most defect-dense subsystem, to add coverage of an
unrelated one.

**Keep relying on the manual walkthrough.** Rejected — §2. It cannot detect
regression, and its output decays into a false guarantee. It remains valuable as
a one-time release gate, which is a different thing.

**A full UI-testing framework.** Rejected: disproportionate, and it would need
its own maintenance, its own flake budget, and a dependency this project has no
other use for.

**Assert on the accessibility tree rather than DOM focus.** Attractive — it
would verify what assistive technology actually receives. Rejected for now: it
needs platform AT APIs, which is a much larger surface than this RFC, and DOM
focus is where every RFC-042 defect actually lived.

## 14. Open questions

1. **How many consecutive green runs before promoting to blocking?** (§10.)
   Proposed: ten on `main`. Arbitrary but written down, which is the point.
2. **Does §7's JavaScript relocation happen inside this RFC or before it?** It
   is a prerequisite for responsible implementation and also independently
   useful. I lean toward before, as its own small task, so this RFC's slices
   start with a working local loop.
3. **Should F (conflict) be in scope at all**, given it is the only item
   requiring the harness to mutate the filesystem under a running app? It is
   also the item with the worst failure consequence. Recorded rather than
   resolved.
