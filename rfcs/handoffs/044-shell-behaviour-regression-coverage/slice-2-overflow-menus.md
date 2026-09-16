# RFC-044 handoff — slice 2: overflow menus

**Governing RFC:** [RFC-044](../../done/RFC-044-shell-behaviour-regression-coverage.md) §8 B, §10, §11
**Slice:** 2 of 3
**Baseline:** `main` after **task 016** merges (moved from task 014,
2026-09-15). Tasks 014 and 016 both add phases to the same phase machine, driver
and milestone list this slice extends. If 016 has not merged, **stop and report**
rather than branching around it. The run's state when your first phase starts
is whatever 016's last phase leaves — expect a new untitled document in Text
mode; read the state rather than assuming it.
**Status:** inherited from RFC-044 (Implemented, on `main`; accepted 2026-08-24)
**Date:** 2026-09-15

---

## 0. How to read this handoff

Sections are **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a mechanism you may replace with reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`.

## 1. Purpose

RFC-042 §7.2 specifies the WAI-ARIA Menu Button pattern for both overflow menus
— the app menu and the editor-tools menu. Every row of that table has been
verified by reading only. Slice 1 built the vehicle; this slice drives the
second contract family through it.

## 2. What already exists · context, verified on `54e3c04`

| Thing | App menu | Editor-tools menu |
|---|---|---|
| Trigger | `id="app-menu-trigger"` (`app_bar.rs:131`) | `id="editor-tools-trigger"` (`editor_header.rs:197`) |
| Trigger ARIA | `aria-haspopup="menu"`, `aria-expanded`, `aria-controls` | same |
| Menu container | `id="app-overflow-menu"`, `role="menu"`, `tabindex="-1"` | `id="editor-tools-menu"`, `role="menu"`, no `tabindex` |
| Items | `role="menuitem"`, `tabindex="-1"` | same |
| Trigger keys | `trigger_key_intent`: Down / Enter / Space → first, Up → last | same |
| In-menu keys | wrap `onkeydown`: Escape → explicit close; `menu_item_key_intent`: Down / Up wrap, Home / End | same |
| Focus movement | `focus_menu_item` — **inside `requestAnimationFrame`** | same |

Two close paths, and the difference is the whole of contracts 6 and 7:

- **Explicit** — Escape, trigger toggle, item activation →
  `release_and_restore_menu_focus`: focus returns to the trigger.
- **Implicit** — the app frame's `onclick` / `onfocusin` (`app.rs:271-276`) →
  `release_menu_focus`: **no restore**. Both menu wraps stop `click` and
  `focusin` from propagating, so this fires only for interaction *outside* the
  menu.

No new production markup is needed, and none may be added. The container
`tabindex` difference between the two menus is recorded, not a contract — leave
it.

## 3. The contracts · **[Binding], for both menus**

1. **Down on the trigger** → menu opens, first item focused, trigger
   `aria-expanded="true"`.
2. **Up on the trigger** → opens, last item focused.
3. **Enter**, and separately **Space**, on the trigger → opens, first item
   focused.
4. **Down on the last item wraps to the first; Up on the first wraps to the
   last.**
5. **Home** → first item; **End** → last item.
6. **Escape** → menu gone, `aria-expanded="false"`, and
   `document.activeElement` **is the trigger**.
7. **Tab closes and does not restore** — mechanism in §4; not a synthetic `Tab`.

Contracts 1–6 are app-intercepted keys, so the slice-1 recipe applies unchanged:
dispatch the key, then assert `document.activeElement`. Focus moves on the next
animation frame — wait for the condition, never assume it is immediate.

## 4. Contract 7 is the A.1 trap again · **[Binding]**

Neither menu has a `Tab` handler. The menu closes on Tab because **native** Tab
advancement moves focus outside the menu wrap and the app frame's `onfocusin`
handler releases the menu. The first half is a browser default action, so a
synthetic `Tab` moves nothing, fires no `focusin`, and closes nothing — exactly
A.1's finding.

RFC-044 §11 originally claimed menus inherited none of that problem. I wrote
that without reading the menu code, and it was wrong. §11 is corrected alongside
this handoff.

**Test bekoedit's half:** with the menu open and an item focused, call `.focus()`
on an element **outside** the wrap — the tree's `tabindex="0"` row is stable in
this run — then assert:

- the menu container is gone and the trigger reads `aria-expanded="false"`;
- `document.activeElement` is **that element**, not the trigger.

Script `focus()` does fire `focusin`, which the slice-1 spike already relied on.
Whether a *real* Tab lands outside the wrap is DOM order plus platform
behaviour, and out of scope for the same reason as A.1.

Build the "focus leaves the menu" step as a reusable driver helper.
RFC-044 §8 D (slice 3) exercises the same close path.

## 5. Never activate a menu item · **[Binding]**

- **"Open Folder"** (`app_bar.rs:177`) and **"Export HTML"**
  (`editor_header.rs:318`) open `rfd::AsyncFileDialog` via xdg-portal. A portal
  dialog **escapes `xvfb` onto the owner's real desktop session** (RFC-042 §10).
  In CI, with no portal, it can hang the run instead. The header's **"Save As"**
  (`editor_header.rs:143`) is the same, and is not a menu item — never click it
  either.
- Every other item — New File, Close Workspace, Settings, Split, Outline,
  Backlinks, History — changes shell state mid-run, after which later phases are
  no longer testing what they claim.

So:

- **No Enter or Space unless `document.activeElement` is a trigger.** Check this
  *before* dispatching, and fail naming the element if it is not. The guard
  turns a sequencing mistake into a named failure rather than a dialog on
  someone's screen.
- No clicks on items. Close through Escape (contract 6) and focus-leave
  (contract 7) only.

## 6. Reaching the editor-tools menu

**Both menus are binding.** The tools trigger lives in `EditorHeader`, and I have
not verified whether it renders without an open document. If it does not, open
one by pressing Enter on a Markdown row — `OpenDocument` opens it, and after task
014 it also moves focus to the editor. Then `.focus()` the tools trigger before
driving it. Mechanism **[Advisory]**; report which case applied.

## 7. Landing it · **[Binding]**

**Extend the existing second run.** No third run mode. Append menu phases after
the current terminal phase (task 016's, once merged): extend
`ShellBehaviourPhase`, `next()`, `expected_milestone`, `EXPECTED_MILESTONES` and
the driver's `phases` list together.

- The pin-registry block and its constants stay untouched.
  `webview-smoke-driver-parity.test.mjs` must pass unmodified.
- `webview_smoke/shell_behaviour.rs` is at 302 ELOC. If menu phases would take it
  past ~450, put them in a submodule instead of squeezing under the gate.
- The step stays `continue-on-error`. **Its promotion clock restarts when this
  merges**, because the step's phase set changed (RFC-044 §10, decided
  2026-09-15). In the same merge, update the clock line beside
  `continue-on-error` in `.github/workflows/ci.yml` with the new start date and
  run 1.

## 8. Determinism · **[Binding]**

Poll for a named condition with a timeout; never sleep. Every timeout reports
which condition never became true — include `document.activeElement` and the
trigger's `aria-expanded` in that report, since those are what will be wrong.

## 9. Non-change scope · **[Binding]**

- Production markup and handlers: `app_bar.rs`, `editor_header.rs`,
  `shell_focus.rs`, and `app.rs`'s close handlers.
- **If a contract fails because the app is wrong, stop and report.** Do not fix
  the app in a coverage slice — contract 7 of slice 1 is the precedent.
- `driver.js`, `--webview-smoke`, the RFC-041 step, the pin-registry block.
- RFC-044 §8 C–F.
- Version numbers, `CHANGELOG.md`, `ROADMAP.md`,
  `docs/src/manual-release-checklist.md`.

## 10. Required tests · **[Binding]**

1. Driver unit tests for every menu phase, using the existing shared fakes and
   harness under `crates/bekoedit-app/js/test/`. Extend them; do not fork them.
2. Rust tests for the new phases, mirroring slice 1's
   `shell_behaviour/tests.rs`.
3. The WebView run: all seven contracts, for both menus.
4. **Three app-code mutations, each proven to fail the contract it targets:**

   | Mutation | Must fail |
   |---|---|
   | `menu_item_key_intent` stops wrapping | contract 4 |
   | Escape uses `release_menu_focus`, so no restore | contract 6 |
   | The app frame's `onfocusin` uses `release_and_restore_menu_focus` — the C3 defect's shape | contract 7 |

   WebView behaviour cannot be observed locally, so these need CI. One probe
   commit carrying all three is fine, **if** each contract fails naming itself —
   the three target distinct contracts. Run it on a throwaway branch
   (**[Advisory]**) so the slice branch's history stays honest, and delete that
   branch afterwards.

The third mutation matters most. It is the defect RFC-042 slice 1 found by
reading, and contract 7 is what would catch its return.

## 11. Acceptance criteria · **[Binding]**

1. All seven contracts, for both menus, in the second run, green in CI.
2. Contract 7 implemented per §4, not with a synthetic Tab.
3. §5's trigger guard present, and no item ever activated.
4. The three §10.4 mutations each fail their contract, named.
5. Parity test unmodified and green; RFC-041 step unchanged and blocking.
6. The promotion clock line updated with its new start, in the merge.
7. No production code changed.

## 12. Required evidence

- The §10.4 probe run URL, with the three named failures quoted from the log.
- Both menus' milestone lists from the green run's log — not the job tick.
- Which §6 case applied to the tools menu.
- Before/after test counts, Rust and JS.

## 13. CI and merge

Branch, commit, push, draft PR — pre-authorized. The refined `main`-moved rule
from slice 1's handoff §13 applies: stop only if the move touches a file this
slice touches, and merge `origin/main` in — never rebase. Integration per
`.git-exclude/governance/merge-procedure.md`.

## 14. Review-request format

`.git-exclude/review-request/<date>-rfc-044-slice-2-overflow-menus.md`, workflow
policy §9.2 sections. **Lead with contract 7's result and the §10.4 probe's three
named failures.** Those four facts are what separate this slice from a suite that
merely runs.
