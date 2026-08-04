# RFC-042 handoff — slice 3: menu and tab keyboard contracts

**Governing RFC:** [RFC-042](../../proposed/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) §7.2, §7.3
**Slice:** 3 of 7 (see RFC-042 §13)
**Depends on:** slice 1 (focus authority), slice 2 (`shell_focus` conventions)
**Baseline:** `main` at `60f9ad8`
**Status:** inherited from RFC-042 (Proposed). **Ready to start.**
**Date:** 2026-08-04

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**, per the convention from
slice 2.

- **[Binding]** — an architecture, security, scope, or conformance decision.
  Mine to make, yours to implement. If one looks wrong, stop and say so.
- **[Advisory]** — a mechanism I am suggesting from outside the code. Replace
  it with something better without asking; state what you did and why.

## 1. Purpose

`app_bar.rs` and `editor_header.rs` declare `role="menu"`, `role="menuitem"`,
`role="tablist"`, and `role="tab"`, and implement none of the keyboard
behaviour those roles oblige (RFC-042 F-4, F-5). Slice 1 added Escape
dismissal, disclosure state, and accessible names; this slice delivers the
navigation.

## 2. Background

Verified on `60f9ad8`:

- **App menu** — `div id="app-overflow-menu" role="menu"` with four
  `role="menuitem"` buttons, one of them behind `if has_workspace`.
- **Editor tools menu** — `div id="editor-tools-menu" role="menu"` with five
  `role="menuitem"` buttons, one behind `if backlinks_available`.
- **Mode tabs** — `nav role="tablist"` with three `role="tab"` buttons (Text,
  Preview from a loop; Form separately), each already carrying
  `aria-selected`.

**Item counts vary at runtime.** That single fact drives §5.3: any Rust-side
list of menu items would have to duplicate those conditionals and could drift
from what is actually rendered — the failure mode slice 2 eliminated by
deriving from one source of truth.

## 3. Applicable RFC and requirements

- RFC-042 §7.2 (Menu Button), §7.3 (Tabs, manual activation), §5
  (declare-or-implement), §6.2 (focus-authority transfer rules)
- RFC-042 F-4, F-5
- RFC-021 accessibility baseline; RFC-000 §11

## 4. Change scope · **[Advisory]**

- `crates/bekoedit-app/src/components/app_bar.rs`
- `crates/bekoedit-app/src/components/editor_header.rs`
- `crates/bekoedit-app/src/shell_focus.rs` — menu/tab focus helpers
- `crates/bekoedit-app/src/tests.rs` — guard assertions
- `crates/bekoedit-app/assets/style.css` — `:focus-visible` styling only

## 5. Required implementation

### 5.1 Menu items are not tab stops · **[Binding]**

Every `role="menuitem"` carries `tabindex="-1"`. Focus is placed
programmatically. The **trigger** is the only tab stop for the menu.

This is the APG menu pattern and it differs deliberately from slice 2's tree:
because Tab *closes* a menu (§5.4), Tab never moves between menu items, so
there is no "roving" zero to track. Do not copy the tree's roving-tabindex
arrangement here.

### 5.2 Menu keyboard contract · **[Binding]**

On the trigger:

| Key | Behaviour |
|-----|-----------|
| Enter / Space / Down | open, focus **first** item |
| Up | open, focus **last** item |

Inside the menu:

| Key | Behaviour |
|-----|-----------|
| Down / Up | move to next / previous item, **wrapping** |
| Home / End | first / last item |
| Escape | close, restore focus to trigger |
| Tab | close, do **not** move focus (§5.4) |

Applies identically to both menus.

### 5.3 Item resolution is DOM-relative · **[Binding]**

Menu navigation resolves against the rendered items, not a Rust-side list.
Given the runtime conditionals in §2, a parallel Rust list would have to
re-encode `has_workspace` and `backlinks_available` and could disagree with
what is on screen.

This is the same principle as slice 2 — one source of truth, read at the moment
of use — reached by the opposite mechanism, because here the DOM is that
source and in the tree it was `visible_rows()`.

Focus position is ephemeral UI state, not canonical document state. Nothing in
RFC-000's invariants is weakened by resolving it in the DOM; those govern
source mutation and filesystem authority.

### 5.4 Tab is implicit dismissal · **[Binding]**

Tab closes the menu, releases shell focus authority, and **does not** restore
focus to the trigger — the user is directing focus onward, and pulling it back
would fight that. Escape is explicit dismissal: close, release, **and** restore.

This is RFC-042 §6.2 as amended after slice 1's C3. Getting it backwards
reintroduces exactly that defect.

`app.rs`'s existing `onfocusin` handler already routes focus-leaving to
`release_menu_focus` (release without restore), so Tab may largely work today.
**Verify it rather than assume it**, and do not add a competing handler.

### 5.5 Tabs: manual activation with roving tabindex · **[Binding]**

Unlike menu items, the tablist **is** a persistent tab stop, so roving applies:
exactly one tab carries `tabindex="0"`, the others `-1`.

| Key | Behaviour |
|-----|-----------|
| Left / Right | move focus between tabs, wrapping |
| Home / End | first / last tab |
| Enter / Space | activate the focused tab |

**Manual activation is required** — arrows move focus only. Automatic
activation would fire one RFC-041 protected command per keystroke, with a
snapshot barrier each time.

The tab carrying `tabindex="0"` is the **selected** tab (the active
`EditorMode`), not wherever focus last landed — so tabbing into the tablist
always lands on the current mode.

### 5.6 Tabs must not disturb source focus · **[Binding]**

Arrow-navigating between tabs must **not** acquire shell focus authority, and
must not cancel or disturb a pending source-focus interaction. The tablist is
persistent UI like the tree, not a focus-owning surface under §6.3.

Note the hazard: those buttons carry `data-source-focus-launch` attributes read
by the focus-guard bundle. Moving DOM focus onto them without activation must
not trigger guard behaviour. **Verify this explicitly** and report what you
found — it is the most plausible way this slice breaks something outside
itself.

### 5.7 Focus helpers · **[Advisory]**

Suggested additions to `shell_focus.rs`, following slice 2's rule that only
compile-time constants and integers reach an eval'd script:

```rust
pub enum MenuFocus { First, Last, Next, Previous }
pub fn focus_menu_item(menu_id: &'static str, position: MenuFocus);
pub fn focus_tab(index: usize);
```

`menu_id` is one of the two existing static ids. Shape, naming, and whether
tabs need a separate helper are yours.

**Do not** generalise `focus_tree_row` or `focus_element` to serve these. That
would edit slice 2 and slice 1 code for tidiness and put three slices in one
diff. Unifying them is a reasonable later cleanup; it is not this slice.

## 6. Non-change scope · **[Binding]**

- Tree navigation (slice 2) — `explorer/` is untouched.
- Conflict, Recovery, Settings metadata (slice 4).
- `focus_element`, `focus_tree_row`, and the four trigger constants.
- The slice-1 focus-authority accessors — consume, do not extend.
- `source_sync/` in any form, including the files still near the ELOC limit.
- Which items the menus contain, what they do, or their order.
- `bekoedit-core`, `bekoedit-fs`, `bekoedit-markdown`, `bekoedit-ui-contract`.

## 7. Required tests · **[Binding coverage, Advisory organization]**

**Be aware this slice is thinner on pure coverage than slice 2, by necessity.**
Tree navigation was a pure function over a Rust-owned list; menu navigation
resolves in the DOM (§5.3), so there is less to test headlessly. Do not
manufacture a Rust-side item list purely to have something to unit-test — that
would reintroduce the drift §5.3 exists to prevent.

What must be covered:

1. **Pure key mapping.** The key → intent mapping (Down→Next, Up→Previous,
   Home→First, End→Last, and the trigger's Down/Up asymmetry) is pure. Extract
   it and test it, small as it is.
2. **Guard assertions**, in the existing style: menu items carry
   `tabindex="-1"`; exactly one tab binds `tabindex` to selection; Escape and
   Tab route to the *correct* dismissal helper (`release_and_restore_menu_focus`
   vs `release_menu_focus`) — this is the C3 regression guard and is the most
   valuable assertion in the set.
3. **No new controller tests** — this slice adds no controller behaviour.

## 8. Documentation · **[Binding]**

No `mvp-acceptance.md` change — no acceptance item covers menu or tab keyboard
behaviour. Do not invent one.

New visible strings need EN and JA arms plus `ALL_KEYS` entries.

## 9. Acceptance criteria · **[Binding]**

1. §5.2 and §5.5 key tables behave as specified in both menus and the tablist.
2. Menu items are not tab stops; the tablist has exactly one.
3. Escape restores focus to the trigger; Tab does not.
4. Arrow-navigating tabs neither activates a mode nor disturbs source focus.
5. Only `&'static str` and integers reach any eval'd script.
6. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo test -p bekoedit --locked` green.
7. The blocking Xvfb WebView regression passes in CI.
8. Every file under 500 ELOC.

## 10. Prohibited shortcuts · **[Binding]**

- No `setTimeout`/sleep to sequence focus.
- No Rust-side mirror of the menu item list (§5.3).
- No editing slice 1 or slice 2 focus helpers (§5.7).
- No `.unwrap()`/`.expect()` on element lookups.
- No weakening or deleting an existing test.
- No `--force` push, ever.

## 11. Known risks · **[Advisory]**

| Risk | Mitigation |
|---|---|
| Escape and Tab wired to the same dismissal helper | §7.2's guard assertion is aimed squarely at this |
| Focus moving onto a `data-source-focus-launch` tab triggers guard behaviour | §5.6; verify and report |
| `editor_header.rs` (306 ELOC) grows past the guideline | Extract the tablist into `editor_header/mode_tabs.rs` if it does — and if anything approaches **500**, stop and raise it rather than absorbing a split into this slice |
| Wrapping misbehaves when a conditional item is absent | DOM-relative resolution (§5.3) makes this structurally hard |

## 12. Required evidence

- Changed-file list; before/after ELOC for both components and `tests.rs`.
- Gate output per §9.6, plus the CI WebView regression result.
- Pure test names and results.
- **An explicit statement on §5.6**: what you did to confirm arrow-navigating
  tabs does not disturb source focus, and what you observed.
- Confirmation that no slice-1 or slice-2 focus helper was modified.
- A manual note if a safe display is available; if not, say so plainly rather
  than omitting the item — do not drive the owner's session (RFC-042 §10).

## 13. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL and result.
Merging, merge mechanism, marking ready, tags, and releases still require
explicit instruction.

Commit scope `app:`; reference RFC-042 slice 3.

## 14. Review-request format

`.git-exclude/review-request/2026-08-04-rfc-042-slice-3-menus-and-tabs.md`, with
the workflow policy §9.2 sections. Lead with the §5.6 finding.
