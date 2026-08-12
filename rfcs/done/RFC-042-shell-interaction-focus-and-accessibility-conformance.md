# RFC-042: Shell Interaction, Focus, and Accessibility Conformance

**Project:** bekoedit
**Status:** Implemented — slices 1–4 shipped in **0.14.0** (tagged 2026-08-10);
slice 5 merged to `main` as `84f2e7e` on 2026-08-12 and ships in the next
release; slice 6 withdrawn; slice 7 completed by task 006. See §13.1.
**Track:** UI/UX stabilization
**Milestone:** v0.14.0
**Priority:** High
**Date:** 2026-07-31
**Related RFCs:** [RFC-010](./RFC-010-main-shell-layout-and-navigation-ux.md), [RFC-021](./RFC-021-accessibility-baseline-and-interaction-contracts.md), [RFC-023](./RFC-023-error-surfaces-status-bar-and-user-feedback.md), [RFC-041](./RFC-041-source-editor-lifecycle-and-synchronization-controller.md)

---

## 1. Summary

Define one conformance contract for the application shell surrounding the
source editor: which composite-widget patterns the shell implements, who owns
keyboard focus at each moment, what accessibility metadata each surface must
expose, and how workspace and editor panes respond to available width.

RFC-021 established an accessibility *baseline* and RFC-010 established the
shell layout. Both declared ARIA roles on composite widgets — `tree`, `menu`,
`tablist` — without the keyboard behavior those roles oblige. This RFC closes
that gap, and resolves the focus-authority question that RFC-041 raised but
scoped out: RFC-041 gave Rust exclusive authority over *source-editor* focus,
while the shell independently moves focus for menus, panels, and screen
replacements. Two uncoordinated focus owners is the defect class RFC-041 was
written to eliminate.

This RFC governs the v0.14.0 theme. Work already in progress on this theme is
in scope and section 11 records the revisions it requires.

## 2. Current state and evidence

Findings below were verified against the working tree at `0.13.1`.

| # | Finding | Evidence |
|---|---------|----------|
| F-1 | `role="tree"` / `role="treeitem"` declared with no arrow-key navigation and no roving tabindex | `crates/bekoedit-app/src/components/explorer.rs:214`, `:284` |
| F-2 | `aria-selected` is absent from the workspace tree | no `aria_selected` anywhere in `explorer.rs`; only `editor_header.rs:98`, `:120` |
| F-3 | `docs/src/mvp-acceptance.md` marks the tree item ✅ citing `aria-selected` evidence that does not exist | acceptance file, "File tree exposes `role="tree"` / `role="treeitem"` with `aria-selected`" |
| F-4 | `role="menu"` / `role="menuitem"` declared with no arrow-key navigation, no focus entry on open, no focus restore on close | `app_bar.rs:84`, `editor_header.rs:220` |
| F-5 | `role="tablist"` / `role="tab"` declared with no Left/Right arrow navigation | `editor_header.rs:89`, `:97`, `:119` |
| F-6 | The conflict banner — the data-loss decision surface — exposes no role, no accessible name, no live region, and receives no focus when it appears | `conflict_banner.rs` contains no `role`/`aria_*` at all |
| F-7 | `settings_screen.rs` and `recovery_screen.rs` replace the whole screen with no roles, no accessible name, and no focus management | both files contain no `role`/`aria_*` |
| F-8 | Form Mode block editing surfaces expose no accessibility metadata | `form_mode.rs`, `form_mode/block_view.rs` contain no `role`/`aria_*` |
| F-9 | `.split-divider` advertises `cursor: col-resize` but no pointer handler exists; nothing in the shell is resizable | `assets/style.css:265`, `split_mode.rs:58`; no `onmousedown`/`onpointerdown` in `src/` |
| F-10 | No `@media` rule exists; explorer is pinned at `width: 250px; min-width: 250px` and the right panel at `min-width: 180px` | `assets/style.css` |

F-3 is a governance defect, not only a code defect: a 1.0.0 acceptance item is
marked complete against evidence that is not present.

F-9 also contradicts `ROADMAP.md`, which lists RFC-010 as having shipped
"resizable sidebar panes". `rfcs/README.md` describes the same RFC accurately
as "explorer collapse". The roadmap line is wrong and section 12 corrects it.

## 3. Goals

- Adopt one named, checkable conformance target for the shell's composite
  widgets rather than per-component judgement.
- Define a single focus-authority rule covering the shell and the RFC-041
  source-editor controller, so exactly one owner holds focus at any moment.
- Specify the keyboard contract for every shell surface that declares a
  composite role.
- Specify required accessibility metadata for every shell surface, including
  the conflict, recovery, and settings surfaces that currently have none.
- Give workspace and editor panes defined behavior under narrow widths, and
  either implement or withdraw the resize affordance.
- Make the resulting behavior testable without a screen reader in CI.
- Correct the acceptance and roadmap statements this work invalidates.

## 4. Non-goals

- Redesigning the visual language, spacing, or color system.
- Certifying screen-reader behavior across all three OS WebViews; that remains
  a manual release item.
- Changing canonical text, patch resolution, save, or conflict *semantics* —
  only how conflict state is presented and announced.
- Re-opening the RFC-041 lifecycle state machine or its protocol version.
- Introducing a general-purpose focus-management framework.
- Multi-tab, multi-window, or multi-root workspaces.

## 5. Conformance target

The shell implements the WAI-ARIA Authoring Practices patterns named below.
Where a surface declares a composite role, it implements that pattern's
keyboard contract in full or it does not declare the role.

| Surface | Pattern | Declared today |
|---------|---------|----------------|
| Workspace file tree | Tree View | yes, incomplete |
| App and editor overflow menus | Menu Button | yes, incomplete |
| Editor mode switcher | Tabs (manual activation) | yes, incomplete |
| Workspace search, new-file row | Disclosure | partial |
| Settings, Recovery | Modal-equivalent screen replacement | no |
| Conflict banner | Alert with actions | no |
| Save status, toasts | Live region | yes, adequate |

"Declare the role or implement the pattern" is the operative rule. A surface
that cannot yet meet its pattern must drop to a non-composite role rather than
present a contract to assistive technology that the code does not honor.

## 6. Focus authority

This is the decision RFC-041 deferred, and the one this RFC exists to fix.

### 6.1 Invariant

At any instant exactly one of two authorities owns keyboard focus:

- the **source authority** — the RFC-041 controller, for CodeMirror focus in
  Text and Split; or
- the **shell authority** — for menus, transient panels, screen replacements,
  and the conflict banner.

Neither authority may issue a focus effect while the other holds authority.

### 6.2 Transfer rules

Authority transfers only at these points, and only through existing
primitives — this RFC introduces no parallel focus manager:

1. **Shell acquires.** Opening any focus-owning shell surface calls
   `cancel_source_focus` (`source_sync/focus.rs`) *before* moving focus. This
   already exists and is already called from several call sites; this RFC makes
   it mandatory and uniform.
2. **While shell holds.** Source focus intents submitted in this window are
   cancelled, not queued. This matches the existing
   `cancel_focus_interactions` behavior; a queued intent that fires after the
   surface closes would steal focus from the restored target.
3. **Shell releases.** Closing a focus-owning surface always releases
   authority. Whether it also *moves* focus depends on why it closed:

   - **Explicit dismissal** — Escape, toggling the trigger, activating an item:
     release **and** restore focus to the invoking element.
   - **Dismissal by focus going elsewhere** — outside click, focus leaving the
     surface: release **only**. The user has already directed focus somewhere;
     moving it back would steal it, and on a focus-in path it would compete
     with the source authority for the same frame.

   The shell never focuses CodeMirror directly.

   *(Clarified 2026-07-31 after slice-1 review finding C3: the original text
   named restore-on-Escape and separately retained outside-click close, without
   stating that restore does not apply to the latter.)*
4. **Source reacquires.** Only through the normal controller path, after a
   validated `Ready` editor exists. Restoring shell focus to a trigger button
   is not a source-focus event and must not schedule one.

### 6.3 Mutual exclusion

At most one focus-owning shell surface is open at a time. Opening any of them —
either overflow menu, the workspace-search disclosure, the new-file disclosure —
closes the others.

This is a load-bearing requirement, not a UX preference: §6.1 is a single
authority, so overlapping surfaces would let one surface's close release
authority that another still needs. Enforce exclusion rather than counting
holders; a counter would mask an unbalanced acquire instead of surfacing it.

*(Added 2026-07-31 after slice-1 review finding M4. Opening search already
closed the menus; opening a menu did not close search.)*

### 6.4 Consequence

A shell surface may not be opened from inside a source-focus guard window, and
a source-focus interaction may not be launched from inside an open shell
surface without closing it first. Implementations must not work around this by
deferring with a timer; a timing workaround here reproduces the RFC-041 class
of bug.

## 7. Keyboard contracts

### 7.1 Workspace tree

Single tab stop with roving tabindex. Tab enters the tree at the active row and
Tab leaves it; it does not step through every file.

| Key | Behavior |
|-----|----------|
| Up / Down | move active row across the flattened visible rows |
| Right | expand a collapsed directory; on an expanded one, move to first child |
| Left | collapse an expanded directory; otherwise move to parent |
| Home / End | first / last visible row |
| Enter / Space | open a file, toggle a directory |

The active row carries `tabindex="0"`; every other row carries `tabindex="-1"`.
The active row carries `aria-selected="true"` (F-2), and the open document's
row is the selected row when it is visible.

**Non-openable rows stay focusable.** They use `aria-disabled="true"` and a
non-activating handler, not the native `disabled` attribute. A natively
disabled element is removed from the tab order and from assistive-technology
focus, which contradicts the tree pattern: a user must be able to perceive that
a non-Markdown file exists. This revises in-flight work; see section 11.

### 7.2 Overflow menus

Menu Button pattern for both the app menu and the editor tools menu.

| Key | Behavior |
|-----|----------|
| Enter / Space / Down on trigger | open and focus the first item |
| Up on trigger | open and focus the last item |
| Up / Down in menu | move between items, wrapping |
| Home / End | first / last item |
| Escape | close and restore focus to the trigger |
| Tab | close and continue normal tab order |

Escape-to-close already landed in the in-flight work; focus entry, roving focus
between items, and focus restore have not. Closing on outside click and on
focus leaving the menu is existing behavior and is retained.

### 7.3 Mode switcher

Tabs with manual activation: arrows move focus between tabs, Enter or Space
activates. Manual activation is required because activating a mode is a
protected command under RFC-041 — automatic activation on arrow would fire a
lifecycle transition per keystroke.

| Key | Behavior |
|-----|----------|
| Left / Right | move focus between mode tabs, wrapping |
| Home / End | first / last tab |
| Enter / Space | activate the focused mode |

### 7.4 Transient panels

Workspace search and the new-file row are Disclosure surfaces. Each has a
trigger exposing `aria-expanded` and `aria-controls`, receives focus on open,
closes on Escape, and restores focus to its trigger on close. In-flight work
has delivered most of this; focus restore on close is the gap.

### 7.5 Screen replacements

Settings and Recovery replace the main shell. Each exposes a landmark role and
an accessible name, moves focus to its heading on entry, and restores focus to
the invoking control on exit. Recovery additionally announces the number of
recoverable documents through a live region, because a user who cannot see the
screen must learn that unsaved work is being offered back.

Focus is not trapped. These are screen replacements, not modal dialogs layered
over live content; there is no background content to trap focus away from.

### 7.6 Conflict banner

The banner is an alert with actions (F-6). It exposes `role="alert"` and an
accessible name.

**It must not move focus when it appears.** `role="alert"` already announces
it to assistive technology without taking focus, which is what the APG
prescribes for a non-modal alert.

*(Amended 2026-08-04. The original text required focus to move to the banner's
first action "because it blocks autosave and demands a decision." That was a
data-loss hazard, and it was my error. Checking what those actions do:*

| Order | Action | Effect |
|---|---|---|
| 1 | Keep my version | `atomic_write` over the disk file — destroys the external change |
| 2 | Reload from disk | `DocumentSession::load` — **discards all unsaved local edits** |
| 3 | Save my version as a copy | preserves both; the only non-destructive option |

*The banner appears in response to an external event, not a user action, and
can arrive mid-keystroke. Placing keyboard focus on action 1 means a stray
Space or Enter overwrites the on-disk version, and one arrow key away destroys
the user's own unsaved work. For a product whose central promise is that
neither version is ever lost silently, an unrequested focus move onto a
destructive control is the wrong default. Announce; do not seize.)*

The banner's actions are reachable by Tab in document order like any other
controls; no special focus handling is required or permitted.

Because focus never moves, the IME-composition guard the original text called
for is unnecessary — there is no focus move to defer. Do not implement one.

**Open question carried to RFC-041 §9.** That RFC describes these actions as
"ordered safest-first," but Save-a-copy — the only option that loses nothing —
is rendered last. Either the ordering or the description is wrong. Out of scope
for this RFC; recorded so it is not lost.

## 8. Accessibility metadata

Every interactive control has an accessible name from a translated key — no
literal strings in RSX. Every icon-only control carries both `aria-label` and
`title`. Every disclosure trigger carries `aria-expanded` and `aria-controls`
naming a real element id. State conveyed by color is also conveyed by text,
icon, or ARIA state.

Form Mode blocks (F-8) expose a group role per block with the block kind in the
accessible name, so a keyboard user can tell a heading field from a paragraph
field without seeing the layout. Raw Markdown Islands announce that they are
raw regions and why, reusing the existing island reason strings.

New keys are added to `ALL_KEYS` with both EN and JA arms; the existing parity
and plain-language guard tests apply unchanged.

## 9. Responsive and resizable layout

### 9. Status — withdrawn 2026-08-10

**This entire section is withdrawn.** §9.1 was resolved toward withdrawal by the
project owner (D-1) and the lying `col-resize` cursor is removed by task 006.
§9.2 is withdrawn with it: narrow-width behaviour is polish with no demand
signal behind it, and a deferral with no trigger is worse than a decision. See
§13.1 for the reopening trigger.

The original text is kept below as the record of what was considered.

### 9.1 Resize

The `col-resize` cursor on a non-draggable 1px divider (F-9) is an affordance
that lies. Resolve it in one of two directions, decided in section 15:

- **Implement.** A draggable divider for the explorer/editor boundary and the
  split boundary, with persisted widths in `AppSettings`, keyboard resize via
  arrows on a focusable separator with `role="separator"` and
  `aria-valuenow`, and a double-click reset.
- **Withdraw.** Remove the `col-resize` cursor and correct `ROADMAP.md`.

### 9.2 Narrow widths

Below a defined shell width the explorer collapses to its existing collapsed
state rather than compressing the editor below a readable measure, and Split
Mode falls back to single-pane Text with the mode still selectable. The
explorer's fixed `min-width: 250px` becomes a maximum-bounded flexible width.
This is a desktop window-size concern, not a mobile target; NG-009 stands.

## 10. Testing strategy

Rendering is not automatically testable in this stack, and RFC-026 deliberately
declined SSR and Playwright harnesses. This RFC does not reverse that. Coverage
is placed where it can be honest:

1. **Pure keyboard-reducer tests.** Extract tree navigation and menu roving
   focus into pure functions over a flattened row list and an item count, and
   test them directly: arrow movement, wrap, expand/collapse, Home/End,
   skip-vs-stop on disabled rows. No DOM required.
2. **Metadata guard tests.** Extend the existing `tests.rs` source-assertion
   pattern to assert each required role, state attribute, and translated label
   is present on its surface. These are coarse, and they are the same technique
   already used for `SearchOpen` and `is_markdown_path`; they catch removal, not
   correctness.
3. **i18n tests.** Existing parity and plain-language guards, extended by the
   new keys.
4. **Focus-authority tests.** Assert at the controller level that opening a
   shell surface cancels pending source-focus interactions and that no source
   focus effect is emitted while shell authority is held.
5. **WebView regression.** Extend the existing RFC-041 `--webview-smoke` driver
   with a keyboard traversal of the tree and one menu, asserting the active
   element after each key. This runs in the existing blocking Xvfb job.
6. **Manual.** Screen-reader spot checks per platform stay in
   `docs/src/manual-release-checklist.md`.

**Where GUI verification may run.** Any check that launches a real window or
sends synthetic input runs in CI, or on a display dedicated to testing. It is
never run against an interactive desktop session, and never against one
belonging to the project owner. Session-wide input tools drive whatever holds
focus, not a chosen window, so a misdirected event lands in unrelated
applications. This matters doubly for RFC-042: scripted GUI automation assumes
windows take focus when expected, which is the very property under test here —
a harness whose failure mode is the defect it is testing for, with someone
else's work in the blast radius, is not a test. Checks that cannot meet this
constraint — anything driving a native OS dialog — are human walkthrough items.

*(Added 2026-07-31 after the slice-1 second re-review surfaced a live
compositor session in the development environment.)*

## 11. Revisions required to in-flight work

Uncommitted work on this theme is in scope and mostly conforms. Two items
require revision before it lands:

1. **`disabled: !is_openable` on tree rows must be reverted** to
   `aria-disabled` with a non-activating handler, per section 7.1. The
   accompanying assertion in `crates/bekoedit-app/src/tests.rs` changes with it.
2. **Tree rows must not each become a tab stop.** The change from `div` to
   `button` is retained for semantics and click behavior, but tabindex is
   managed per section 7.1.

Everything else in the in-flight change — Escape dismissal, disclosure state,
`aria-controls`, translated menu labels, new-file focus and cancel — is
consistent with this RFC and needs no rework.

## 12. Corrections to existing documents

This RFC's implementation must also correct, in the same change series:

- `docs/src/mvp-acceptance.md` — the file-tree accessibility item is not
  satisfied (F-3). Mark it accordingly until section 7.1 lands.
- `ROADMAP.md` — RFC-010 did not ship resizable sidebar panes (F-9).
- `docs/src/architectural-invariants.md` — cites RFC-000 at a `proposed/` path
  though it lives in `done/`, and is absent from `docs/src/SUMMARY.md`, so
  mdBook never renders it.

## 13. Implementation slices

Independently reviewable, in dependency order:

1. Focus authority: uniform `cancel_source_focus` acquisition, restore-to-trigger
   on close, controller-level tests (section 6).
2. Tree: roving tabindex, arrow navigation, `aria-selected`, `aria-disabled`
   revision, pure-reducer tests (section 7.1, F-1, F-2, F-3).
3. Menus and tabs: focus entry, roving focus, restore, arrow navigation
   (sections 7.2, 7.3).
4. Conflict, Recovery, Settings metadata and focus entry (sections 7.5, 7.6).
5. Form Mode block metadata (section 8).
6. Responsive and resize disposition (section 9).
7. Document corrections (section 12).

Slices 1 and 2 carry the theme's value.

### 13.1 Slice status — recorded 2026-08-10

Owner-approved disposition at the RFC-042 checkpoint. Recorded here rather than
left as an open "deferred" note, because an unbounded deferral is how an RFC
rots in `proposed/` — the failure this project has already corrected once.

| Slice | Status |
|---|---|
| 1 | **Implemented** — v0.14.0 |
| 2 | **Implemented** — v0.14.0 |
| 3 | **Implemented** — v0.14.0 |
| 4 | **Implemented** — v0.14.0 |
| 5 | **Implemented** — `84f2e7e`, 2026-08-12; not in 0.14.0, ships next release |
| 6 | **Withdrawn** — see below |
| 7 | **Implemented** — completed by task 006 |

*Slice 5 closed this RFC. Updated 2026-08-12 when it merged; the RFC moved to
`done/` in the same change.*

**Why slice 5 is not in 0.14.0.** It merged after the `0.14.0` tag, deliberately.
That release's CHANGELOG and ROADMAP both state that Form Mode block editing
exposes no accessibility metadata, and merging slice 5 first would have made the
tag contain work its own notes deny.

**Slice 5 is scheduled, not deferred.** Form Mode is one of three primary
editing modes and currently carries no accessibility metadata at all. Leaving it
unlabeled while the tree, menus, tabs, conflict banner, Recovery, and Settings
all conform is an incoherent end state, and it would undercut this RFC's own
declare-or-implement rule. It needs no new design — section 8 already specifies
it. **This RFC stays in `proposed/` until slice 5 is implemented**; it is
unfinished work, not deferred work.

**Slice 6 is withdrawn in full.** Its resize half was already resolved toward
withdrawal (D-1) and removed by task 006. Its remaining half — narrow-width
behaviour, section 9.2 — is polish with no demand signal and no user report
behind it. Withdrawing it honestly is better than a deferral nobody revisits.

*Reopening trigger, should one be wanted:* a user reports the layout unusable at
their window size. Absent that, section 9 is closed.

**Release note.** v0.14.0 shipped slices 1–4, which was its approved scope. This
RFC's disposition is a portfolio question and is not a release gate; slice 5 is
expected in a later release.

## 14. Alternatives considered

**Keep native `disabled` on non-openable rows.** Simpler, and the browser
handles activation suppression. Rejected: it removes rows from assistive
technology's focus order, so a keyboard user cannot discover that a
non-Markdown file exists in the folder.

**Automatic tab activation on arrow.** The more common Tabs variant. Rejected:
mode switches are RFC-041 protected commands; one lifecycle transition per
arrow keypress is both wasteful and a source of spurious barrier holds.

**Trap focus in Settings and Recovery.** Rejected: they replace the shell
rather than layering over it, so there is no background content to trap away
from, and a trap adds an escape-hatch failure mode for no gain.

**Add a shell-wide focus manager.** Rejected: a second focus authority is the
defect RFC-041 was written to remove. Section 6 arbitrates between the two
existing owners instead of adding a third.

**Adopt an off-the-shelf ARIA widget library.** Rejected: the shell is Dioxus
RSX, not a JS component tree; importing a JS widget layer would reintroduce
DOM-owning JavaScript that RFC-002 and RFC-041 deliberately constrain.

**Defer accessibility to post-1.0.** Rejected: RFC-000 §11 and product
principle 6.7 make accessibility part of correctness, and RFC-021 already
committed the roles. The current state promises conformance in markup that the
behavior does not deliver, which is worse than not promising it.

## 15. Open questions

1. **Resize: implement or withdraw?** (Section 9.1.) Implementing adds a
   persisted setting, a pointer-drag path, and a keyboard separator contract —
   real scope. Withdrawing is a two-line change plus a roadmap correction.
   *Recommendation: withdraw for v0.14.0, and reopen as its own RFC if users
   ask.* Owner decision required.
2. **Narrow-width breakpoint.** Section 9.2 defines the behavior but not the
   threshold. Proposed: collapse the explorer below 900 px shell width, drop
   Split to single-pane below 1100 px. *Recommendation: accept as defaults and
   revisit on report.* Architecture decision, recorded here for visibility.
3. ~~**Does this theme gate 1.0.0?**~~ **Resolved 2026-07-31 by the project
   owner: slices 1–4 complete before the 1.0.0 acceptance review.**

   Considered and rejected: (a) downgrading the acceptance item and shipping
   1.0.0 with a stated accessibility gap — rejected because `README.md`
   advertises the tree roles as a shipped feature, so the false claim would
   move to the front page rather than disappear; (b) narrowing to the tree
   alone — rejected because it scopes by what the acceptance checklist happens
   to mention, and that checklist is precisely what F-3 proved unreliable. The
   conflict banner (§7.6, F-6) carries no acceptance line yet sits closer to
   this product's central promise than the file tree does, and narrowing would
   also leave the §6 focus model half-built across two releases.

   Slices 5–7 remain outside v0.14.0.
