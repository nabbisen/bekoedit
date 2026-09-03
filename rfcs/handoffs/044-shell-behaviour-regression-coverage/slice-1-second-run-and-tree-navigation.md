# RFC-044 handoff — slice 1: the second run, and tree navigation

**Governing RFC:** [RFC-044](../../accepted/RFC-044-shell-behaviour-regression-coverage.md) §5, §8 A, §10
**Slice:** 1 of 3 (see §1.1 — the RFC's A+B pairing is split)
**Baseline:** `main` after task 012 merges (`3655df2`). Task 012 adds the DOM
fake this slice's tests depend on; if it has not merged, **stop and report**.
**Status:** inherited from RFC-044 (Accepted 2026-08-24)
**Date:** 2026-09-03

---

## 0. How to read this handoff

Sections are **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a mechanism you may replace with reasoning, without asking.

## 1. Purpose

RFC-042 shipped keyboard contracts for the workspace tree, overflow menus and
mode tabs across five slices. **Every one of them was verified by reading
code.** RFC-044 exists to execute them instead. This slice builds the vehicle
and drives the first family.

### 1.1 The RFC's A+B pairing is split, deliberately

RFC-044 §8 says "A and B are the bulk of the untested surface and should land
first." I am splitting them: **this slice is A (tree navigation) only.**

The reason is that most of this slice is not A. It is a second run, a shared
transport extraction, and a CI gate with a promotion schedule. Landing B in the
same change would mean the first exercise of all that new machinery also carries
two contract families, and if something in the vehicle is wrong the diagnosis
gets harder in proportion. A alone is enough to prove the vehicle: it has seven
sub-contracts, roving tabindex, and a real focus assertion.

B follows immediately as slice 2, against a vehicle that has already run green.
RFC-044 §8 is amended to record this.

## 2. Prove the load-bearing assumption first · **[Binding]**

**Everything in RFC-044 §8 rests on one untested premise: that a synthetic
`KeyboardEvent` dispatched from the driver reaches a Dioxus `onkeydown`
handler.**

The existing driver only ever dispatches `MouseEvent` at elements. Keyboard
contracts need `dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown",
bubbles: true }))` to be seen by the Rust-side handler, through Dioxus's own
event plumbing, in a real WebView.

I have not verified this and neither has anyone else. If it does not work, or
works only with a particular event shape, **the whole of §8 A–F changes** and
this handoff's remaining sections are moot.

**So: prove it first, in the smallest form that proves it.** One key dispatched
at one focused element, one observable consequence. Before the transport work,
before the phase machine, before any contract.

**If it does not work, stop and report.** Do not work around it — that is a
design finding that goes back to the RFC, and possible answers (a different
event construction, a test-only input path, abandoning synthetic keys for this
approach entirely) are not yours to pick alone.

## 3. Share the transport; do not copy it · **[Binding]**

The second run needs the same evaluator-pin handshake the RFC-041 run uses:
exchange ids, `PhaseRequest`/`PhaseAcknowledgement`/`PhaseCompletion`, the
`__bkWebViewSmokeEvalPin` registry, and `run_driver_phase`'s join sequence.

**Extract that transport so both runs use one copy.** It carries this comment:

> Audited against Dioxus Desktop/Document 0.7.9. `NativeDioxusChannel::close`
> only clears the JS queue; its `FinalizationRegistry` emits the query drop…
> Re-audit `native_eval.ts`, `query.rs`, `document.rs`, and `dioxus-document`
> `eval.rs` before updating Dioxus.

Two copies means two re-audits on the next Dioxus upgrade, and one of them will
be missed. That is the whole reason this is binding rather than advisory.

**What is shared vs. what is not:**

| Shared — extract | Not shared — the second run gets its own |
|---|---|
| `run_driver_phase`, the pin/exchange types, completion validation | The phase enum and its order |
| The JS-side pin registry protocol in `driver.js` | The phase bodies, selectors, assertions |
| | `EXPECTED_MILESTONES`, terminal-stage validation |

`SmokePhase`, `PhaseMachine`'s transition table, and `validate_driver_result`
are RFC-041 semantics. Generalise the transport over the phase type; leave the
semantics where they are.

**The extraction must be a move, not a redesign · [Binding].** RFC-044 §12.1
requires the RFC-041 regression to be unchanged and still blocking. A mechanical
extraction makes that provable by inspection; a rewrite does not.

## 4. The second run · **[Binding]**

A **separate run**, per RFC-044 §5 — not more phases bolted onto the RFC-041
sequence. That RFC's regression protects the most defect-dense subsystem in the
project and its milestone contract is a gate; lengthening it changes what a
failure means.

- Its own launch flag, alongside `--webview-smoke`. Naming **[Advisory]**.
- Its own driver JavaScript, its own phase set, its own milestone list.
- Its own CI step (§6).
- `--webview-smoke` and `driver.js` **unchanged**.

It may and should reuse the disposable `Isolated` profile machinery. **This is
what RFC-043 was for** — seed the profile's recents file, set
`reopen_last_workspace`, and the run lands in the shell rather than the Start
Screen, with no dialog. Use it; it is the reason that RFC was built.

## 5. Coverage — §8 A, all seven · **[Binding]**

From RFC-044 §8 A, each an assertion about real focus after a real key:

1. Tab reaches the tree at **exactly one** stop — the roving-tabindex target,
   not every row.
2. Down / Up move the active row.
3. Right expands a collapsed row, then enters it.
4. Left collapses an expanded row, then ascends.
5. Home / End reach first and last.
6. A non-openable row is reachable and not skipped.
7. Enter opens a document **and the editor takes focus**.

`[data-tree-row]` already exists as a stable selector and rows carry
`role="treeitem"` with roving `tabindex` — **no new production markup is needed
for A**, and none should be added. If you find a contract that cannot be
observed without new markup, report it rather than adding an attribute; that is
a finding about the contract's testability.

Assert **`document.activeElement`**, not merely that a class or attribute
changed. RFC-042's defects lived in focus, and a test that checks the styling
that usually accompanies focus will pass when focus is wrong.

**Corrected 2026-09-03 — contract 1 is the exception, and this rule as written
sent it down a dead end.** A synthetic `Tab` cannot drive focus at all:
script-dispatched events do not get browser default actions, and Tab
advancement is one. Contract 1's mechanism is the **roving-tabindex invariant,
asserted live after each of contracts 2–5's key presses** — exactly one
`[data-tree-row]` at `tabindex="0"`, every other at `"-1"`, that row being the
active row and moving with it, and that row `.focus()`-able. See RFC-044 §8 A.1.

For contract 1 the attribute *is* the contract; the rule above stands unchanged
for the other six. My error, found by the dev team while designing contract 1.

## 6. Landing it — non-blocking, with a dated promotion · **[Binding]**

Per RFC-044 §10: land the new step **non-blocking** (`continue-on-error: true`),
and promote it to blocking after **ten consecutive green runs on `main`**
(§14 Q1, settled).

**The promotion must be scheduled when the non-blocking period begins, not left
as an intention.** `ci.yml` carried a stale `continue-on-error: true` long after
the flag it guarded shipped, and it took an audit to notice. Record in the
review request: the date the clock starts, the criterion, and where the count
will be tracked. A `continue-on-error` with no written expiry is how a gate
becomes decoration.

The RFC-041 regression stays blocking throughout.

## 7. Determinism · **[Binding]**

Inherited from RFC-044 §10 and the existing harness:

- Poll for a condition with a timeout. **Never sleep.**
- Every wait names what it is waiting for.
- A timeout reports which condition never became true, not just "timed out".

## 8. Also in scope — the task-012 carry-over · **[Binding]**

Task 012's review scheduled this here rather than leaving it to be remembered.

`js/test/webview-smoke-driver-phases.test.mjs` duplicates `FakeDioxus`,
`request`, `acknowledgement` and `runDriver` from
`webview-smoke-driver.test.mjs`, and the two `request()` copies **have already
diverged** — one parameterises `phase`, the other hardcodes `"launch"`.

Consolidate them into the shared test module alongside the DOM fake, and have
both existing files import from it. This slice will add a third consumer, which
is exactly when the drift stops being theoretical.

Mechanical change, no behaviour: the protocol test file's own mutations must
still fail it afterwards. Demonstrate that.

## 9. Non-change scope · **[Binding]**

- `--webview-smoke`, `driver.js`, and the RFC-041 regression's CI step.
- `SmokePhase`, `PhaseMachine`'s transitions, `validate_driver_result` — except
  as §3's mechanical extraction requires.
- Production markup, except where §5 says report-don't-add.
- RFC-044 §8 items **B–F**. B is slice 2. Landing "just the menu trigger" here
  is how a slice becomes two slices in one review.
- `webview_smoke/tests.rs` — 453 ELOC, 47 of headroom. Adding is allowed but
  watch the gate; prefer a new module.
- Version numbers, `CHANGELOG.md`, `ROADMAP.md`.
- `docs/src/manual-release-checklist.md` — the owner is editing it.

## 10. Required tests · **[Binding]**

1. The §2 spike, kept as a test if it survives as one.
2. Each of §5's seven contracts, individually.
3. The extracted transport's existing Rust and JS tests still pass **unchanged**
   — that is the evidence §3's move was a move.
4. §8's consolidation demonstrated non-breaking.

Every new assertion proven able to fail: break the behaviour, watch the check
fail naming what broke, restore. For the JS half use task 012's mutation method
against a scratch copy. **A test that cannot be shown to fail is not evidence**,
and it applies with extra force to a suite whose purpose is catching what other
tests cannot.

## 11. Acceptance criteria · **[Binding]**

1. §2 proven, or reported and stopped.
2. The RFC-041 regression unchanged and still blocking — its own tests pass
   untouched, and its CI step is green on the same run.
3. A second run exists, lands in the shell via a seeded profile, and covers all
   seven §5 contracts against `document.activeElement`.
4. New step non-blocking, with a written, dated promotion criterion.
5. No sleeps; every wait is a named condition with a timeout.
6. Every new assertion proven able to fail.
7. §8's consolidation done, both existing test files importing from the shared
   module.
8. No production markup added; no B–F coverage.

## 12. Required evidence

- The §2 result **first**, whatever it is.
- The seven contracts with their individual failure demonstrations.
- Before/after test counts, Rust and JS.
- Proof the RFC-041 regression is untouched: its diff, and its CI step green.
- The promotion criterion, with its start date.
- CI run URL, and the new step's own output at step level — not just the job's
  tick.

## 13. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL. Merging,
tags and releases require explicit instruction. If `main` has moved, report the
topology and stop.

Commit scopes: `refactor:` for §3's extraction, `test:` for the run and its
coverage, `ci:` for the workflow step. Split as they split cleanly — the
extraction landing as its own reviewable commit matters here, because "it was a
pure move" is a claim a reviewer should be able to check in isolation.

## 14. Review-request format

`.git-exclude/review-request/<date>-rfc-044-slice-1-second-run-and-tree-navigation.md`,
workflow policy §9.2 sections. **Lead with §2.** If the synthetic-key assumption
does not hold, that is the whole review and the rest of this handoff is void.
