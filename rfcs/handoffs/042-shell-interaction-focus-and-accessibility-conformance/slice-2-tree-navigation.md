# RFC-042 handoff — slice 2: workspace tree navigation

**Governing RFC:** [RFC-042](../../proposed/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) §7.1
**Slice:** 2 of 7 (see RFC-042 §13)
**Depends on:** slice 1 (focus authority) — specifically the `shell_focus`
module, to which this slice adds `focus_tree_row` (§7.5). Slice 2 does not call
`focus_element` and must not modify it.
**Status:** inherited from RFC-042 (Proposed — do not start until the RFC is approved and slice 1 is merged)
**Date:** 2026-07-31

---

## 1. Task title

Make the workspace file tree conform to the WAI-ARIA Tree View pattern.

## 2. Purpose

`explorer.rs` declares `role="tree"` and `role="treeitem"` and honors neither.
In released `0.13.1` the rows are `<div>` with no `tabindex`, so a keyboard-only
user cannot reach the file tree at all; assistive technology is told "tree,
item, collapsed" and then finds that arrow keys do nothing and no row reports
as selected.

This slice delivers the pattern the markup already promises, and closes the
acceptance item corrected at `docs/src/mvp-acceptance.md` (RFC-042 F-2, F-3).

## 3. Background

The tree renders from `dioxus_swdir_tree::DirectoryTree`. `visible_rows()`
already returns the flattened, depth-annotated list of currently visible rows —
`Vec<(TreeNode, u32)>` at `components/explorer.rs:112` — which is exactly the
model tree navigation needs. Navigation is therefore a pure function over that
list plus an active index; it does not require the DOM, and it must be tested
without one.

Per DEC-011 the project deliberately bypasses the library's
`DirectoryTreeView` renderer because its drag handler races on Desktop repaint.
That constraint stands: **nothing in this slice may reintroduce the library's
drag path.**

## 4. Applicable RFC and requirements

- RFC-042 §7.1 (normative for this slice), §5 (declare-or-implement rule)
- RFC-042 F-1, F-2, F-3
- RFC-021 accessibility baseline; RFC-000 §11; product principle 6.7
- DEC-011 (no library drag path)

## 5. Change scope

- `crates/bekoedit-app/src/components/explorer.rs`
- `crates/bekoedit-app/src/components/explorer/tree_nav.rs` — **new**, pure
  navigation logic
- `crates/bekoedit-app/src/components/explorer/tree_nav/tests.rs` — **new**
- `crates/bekoedit-app/src/shell_focus.rs` — add `focus_tree_row` (§7.5); do
  not modify `focus_element` or the trigger constants
- `crates/bekoedit-app/src/tests.rs` — guard assertions
- `crates/bekoedit-app/src/i18n.rs` — only if new visible strings are required
- `crates/bekoedit-app/assets/style.css` — focus-visible styling for the active
  row; no layout change

Module layout follows the project rule: `explorer.rs` plus an `explorer/`
subdirectory, no `mod.rs`, tests in a sibling `tests.rs` rather than `#[test]`
blocks inside the implementation file.

## 6. Non-change scope

- Menu and tab keyboard navigation (slice 3).
- Conflict, Recovery, Settings metadata (slice 4).
- The focus-authority accessors from slice 1 (`acquire_shell_focus`,
  `release_shell_focus`, `shell_focus_held`) — consume them, do not extend or
  modify them. Adding `focus_tree_row` to `shell_focus.rs` per §7.5 is the one
  permitted addition; `focus_element` itself stays exactly as slice 1 left it.
- `dioxus-swdir-tree` version, its drag machinery, or `DirectoryTreeView`.
- Tree scanning, lazy loading, Git status badges, or the context menu.
- `bekoedit-core`, `bekoedit-fs`, `bekoedit-markdown`.
- Search panel, new-file form.

## 7. Required implementation

### 7.1 Pure navigation module

`tree_nav.rs` exposes navigation as pure functions over a row view. Suggested
shape — adapt names, keep the purity:

```rust
pub struct NavRow { pub is_dir: bool, pub is_expanded: bool, pub depth: u32 }

pub enum NavKey { Up, Down, Left, Right, Home, End }

pub enum NavOutcome {
    Move(usize),        // new active index
    Expand(usize),      // caller toggles, then re-derives rows
    Collapse(usize),
    None,
}

pub fn navigate(rows: &[NavRow], active: usize, key: NavKey) -> NavOutcome;
```

Behavior per RFC-042 §7.1:

| Key | Outcome |
|-----|---------|
| Up / Down | move one visible row; clamp at the ends (do not wrap) |
| Right on collapsed dir | `Expand` |
| Right on expanded dir | move to first child |
| Right on file | `None` |
| Left on expanded dir | `Collapse` |
| Left on collapsed dir or file | move to parent (nearest preceding row of lower depth) |
| Home / End | first / last visible row |

Rows that are not openable are **still navigable** — navigation never skips
them. Only activation is suppressed.

### 7.2 Roving tabindex

Exactly one row carries `tabindex="0"`; every other row carries
`tabindex="-1"`. Tab enters the tree at the active row and Tab leaves it. Do
not leave every row in the tab order — with a real workspace that makes
traversing the shell by keyboard unusable, which is why RFC-042 §7.1 requires
roving.

The active row is tracked by **path**, not index, so it survives a rescan,
expand, collapse, or refresh that renumbers rows. When the tracked path is no
longer visible, fall back to the nearest surviving ancestor, then to the first
row.

### 7.3 Active vs selected

These are two different things and both are required:

- **Active** — the roving focus target. Moves with arrow keys. Carries
  `tabindex="0"`.
- **Selected** — the currently open document's row, when visible. Carries
  `aria-selected="true"`; every other row carries `aria-selected="false"`.

Do not conflate them. Moving focus through the tree must not change which
document is open, and opening a document must not require focus to be in the
tree.

You may implement selection with a local signal or via the library's
`on_selected`; that is your call. If you use the library path, verify it does
not pull in the drag machinery (DEC-011) and say so in the review request.

### 7.4 Revision to in-flight work

The uncommitted change added `disabled: !is_openable` to the row button and an
assertion for it in `tests.rs`. **Both must be reverted.** A natively disabled
element leaves the tab order and assistive-technology focus, so a user could
not perceive that a non-Markdown file exists — which contradicts §7.1.

Replace with:

- `aria_disabled: "true"` on non-openable rows;
- the row stays focusable and navigable;
- the activation handler returns early for non-openable rows.

Keep the change from `div` to `button` — that part is correct and stays.

### 7.5 Activation and focus

- Enter or Space: toggle a directory, open a file, no-op on a non-openable row.
- Opening a file hands focus onward through the normal controller path. The
  tree does not focus CodeMirror directly (RFC-042 §6.2 rule 3).

**Focus rows by index, not by id.** *(Revised 2026-07-31 — see the note below;
the original instruction here is superseded.)*

Add to `shell_focus.rs`:

```rust
/// Focus the nth element matching `[data-tree-row]`, on the next frame.
/// Only a `usize` is interpolated, so no caller-controlled text reaches the
/// script.
pub fn focus_tree_row(index: usize);
```

Rows carry a bare `data-tree-row` attribute — no per-row id at all. Arrow-key
movement resolves the active path to its position in the current
`visible_rows()` list and calls `focus_tree_row(position)`.

**Why this replaces the original instruction.** Slice 1 tightened
`focus_element` to `&'static str` (review item R5). A path-derived id is a
runtime `String`, so the original §7.5 text no longer compiles — and rather
than widen `focus_element` back out or bolt on a sanitizing newtype, indexing
removes the problem at the root: an integer cannot carry an injection payload,
so there is nothing to sanitize and no way to get the sanitizing wrong.

This composes with §7.2 rather than conflicting with it: **state** still tracks
the active row by path, so it survives rescans; **focus** resolves that path to
a position at render time. Roving tabindex uses real DOM focus rather than
`aria-activedescendant`, so per-row ids are not needed for accessibility
either.

## 8. Required tests

**Pure tests** (`explorer/tree_nav/tests.rs`) — the substance of this slice:

1. Up/Down move one row and clamp at both ends.
2. Right on a collapsed directory returns `Expand`; on an expanded directory
   moves to the first child; on a file returns `None`.
3. Left on an expanded directory returns `Collapse`; on a collapsed directory
   or file moves to the parent across a multi-level depth gap.
4. Left at depth 0 returns `None`.
5. Home/End reach the first and last rows.
6. Non-openable rows are traversed, not skipped.
7. Navigation over an empty row list returns `None` and never panics.
8. Active-path recovery: when the active path disappears, resolution falls back
   to the nearest ancestor, then to the first row.

**Guard tests** (`crates/bekoedit-app/src/tests.rs`), matching the existing
`include_str!` style: `explorer.rs` contains `aria_selected`, contains
`aria_disabled`, and no longer contains `disabled: !is_openable`.

No SSR, hook, or Playwright harness — RFC-026 declined them.

## 9. Required documentation updates

- `docs/src/mvp-acceptance.md`: restore the file-tree item to ✅ **only when
  this slice is complete**, citing the new pure tests as evidence. The item is
  currently ⚠️ with a dated correction; replace that text, do not delete the
  correction history.
- Any new visible string needs both EN and JA arms plus an `ALL_KEYS` entry.

## 10. Acceptance criteria

1. Arrow, Home, End, Enter, and Space behave per RFC-042 §7.1.
2. Exactly one row is in the tab order at any time.
3. The open document's row reports `aria-selected="true"`; others report false.
4. Non-openable rows are focusable, announced as disabled, and not activatable.
5. The active row survives expand, collapse, and rescan.
6. Navigation logic is covered by pure tests that require no DOM.
7. No library drag path is reintroduced (DEC-011).
8. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --locked`, and `cargo test -p bekoedit --locked` are green.
9. The blocking Xvfb WebView regression still passes.
10. Touched files stay under the 500-ELOC gate; prefer under 300. `explorer.rs`
    is already 311 lines — if it grows past the guideline, split it rather than
    letting it drift toward the hard limit.

## 11. Prohibited shortcuts

- Do not put navigation logic inline in the RSX closure. It goes in
  `tree_nav.rs` and is tested there; an untestable reducer is the whole thing
  this slice is meant to avoid.
- Do not restore the native `disabled` attribute (§7.4).
- Do not make every row a tab stop.
- No `setTimeout`/sleep to sequence focus.
- No `.unwrap()`/`.expect()` on row lookups or element lookups.
- Do not weaken or delete an existing test to make this pass.
- No commits, tags, or pushes.

## 12. Compatibility and security constraints

- No bridge protocol change; no payload shape change.
- No new filesystem, network, or process access.
- **No caller-controlled text may reach an eval'd script.** §7.5's
  index-based focus is designed so this cannot happen: only a `usize` is
  interpolated. Do not reintroduce path-derived element ids, do not widen
  `focus_element` beyond `&'static str`, and do not add a
  sanitize-then-interpolate helper. A file named with quotes or angle brackets
  must not be able to reach the emitted script at all — not "be escaped on the
  way through".

  If you find a requirement that seems to need a per-row id, **stop and
  escalate** rather than solving it locally. That requirement would be a
  design change, not an implementation detail.

## 13. Known risks

| Risk | Mitigation |
|---|---|
| Active row lost after a rescan renumbers rows | Track by path with the fallback chain in §7.2; pure test 8 |
| Roving tabindex fights the browser's native focus after activation | Move focus explicitly on activation; cover in the manual check |
| Index drifts between the Rust-side row list and the DOM order | Both derive from the same `visible_rows()` render; assert the `data-tree-row` count matches the row count before focusing |
| `explorer.rs` drifting toward the ELOC hard limit | Split into `explorer/` submodules as the module layout already anticipates |
| Scope creep into menus or tabs | §6 is explicit; those are slice 3 |

## 14. Required evidence

- Changed-file list.
- Output of the gates in §10.8, plus the WebView regression result.
- The pure test names and results.
- A manual note: Tab into the tree, arrow to a nested file, Enter to open it,
  confirm the editor takes focus and that Shift+Tab returns to a single tree
  tab stop. If no safe display is available, say so plainly rather than
  omitting the item — do not drive the owner's session (RFC-042 §10).
- Confirmation that no caller-controlled text reaches an eval'd script (§12),
  and that `focus_element` was not widened.

## 15. Required review-request format

Write to
`.git-exclude/review-request/2026-07-31-rfc-042-slice-2-tree-navigation.md`
with the sections required by the workflow policy §9.2: implementation summary,
addressed requirements, changed files, important implementation decisions,
differences from the approved design, executed tests, test results, build and
static-analysis results, unresolved issues, known limitations, requested review
focus.

Call out explicitly: the selection mechanism chosen in §7.3 and whether it
touches the library drag path, and confirmation of §12 (no caller-controlled
text in any eval'd script; `focus_element` unchanged).
