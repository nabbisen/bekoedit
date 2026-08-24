# RFC-042 handoff — slice 1: focus authority

**Governing RFC:** [RFC-042](../../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) §6
**Slice:** 1 of 7 (see RFC-042 §13)
**Status:** inherited from RFC-042 (Implemented). Historical execution guide — see RFC-042 §13.1 for this slice's disposition.
**Date:** 2026-07-31

---

## 1. Task title

Establish a single focus authority shared by the application shell and the
RFC-041 source-editor controller.

## 2. Purpose

Today two independent parties move keyboard focus: the RFC-041 controller
(CodeMirror focus, via `source_sync/focus.rs`) and shell components (menus,
panels, screen replacements). Neither knows when the other holds focus.
RFC-041 was written to eliminate exactly this class of ambiguity for the source
editor; this slice closes the remaining half.

After this slice, opening a shell surface reliably takes authority, closing it
reliably returns focus to the control that opened it, and a source-focus intent
issued while the shell holds authority is cancelled rather than firing later
into a surface that has already closed.

## 3. Background

`cancel_source_focus(sync)` (`crates/bekoedit-app/src/source_sync/focus.rs:22`)
already exists and already does the right thing — it cancels pending focus
interactions and cancels the corresponding JS focus guards. It is simply not
called uniformly, and there is no counterpart for releasing authority.

Verified call sites today:

| Location | Surface | Correct? |
|---|---|---|
| `components/app_bar.rs:92` | "Open workspace" menu *item* | yes |
| `components/editor_header.rs:152` | mode control | yes |
| `components/editor_header.rs:254,270,286,302` | tools menu *items* | yes |
| `components/explorer.rs:131` | search toggle | yes |
| `components/start_screen.rs:50` | workspace open | yes |

Verified gaps — surfaces that take focus without acquiring authority:

| Location | Surface |
|---|---|
| `components/app_bar.rs:72` | app menu **trigger** (opens the menu itself) |
| `components/editor_header.rs:211` | editor tools menu **trigger** |
| `components/explorer.rs:150` | new-file disclosure toggle |
| Settings entry point | screen replacement |

And there is no focus restore on close anywhere in `crates/bekoedit-app/src/`.

## 4. Applicable RFC and requirements

- RFC-042 §6 (focus authority — the normative text for this slice)
- RFC-042 §7.2, §7.4, §7.5 (the close/restore obligation this slice enables)
- RFC-041 §5.3 single-owner invariant, §6.3 controller host lifetime
- RFC-021 accessibility baseline; RFC-000 §11

## 5. Change scope

- `crates/bekoedit-app/src/source_sync/controller.rs` (and
  `controller/interaction.rs`) — add explicit shell-authority state and typed
  accessors.
- `crates/bekoedit-app/src/shell_focus.rs` — **new**, shell-side focus helper.
- `crates/bekoedit-app/src/main.rs` — register the new module.
- `crates/bekoedit-app/src/components/app_bar.rs`
- `crates/bekoedit-app/src/components/editor_header.rs`
- `crates/bekoedit-app/src/components/explorer.rs`
- `crates/bekoedit-app/src/components/settings_screen.rs`
- `crates/bekoedit-app/src/source_sync/controller/tests.rs` — authority tests.
- `crates/bekoedit-app/src/tests.rs` — guard assertions.
- `crates/bekoedit-app/assets/style.css` — only if a trigger needs a stable id
  hook; no visual change.

## 6. Non-change scope

Explicitly **do not** touch:

- the lifecycle reducer (`source_sync/lifecycle.rs`, `lifecycle/transitions.rs`)
  or any `LifecycleState` / `LifecycleEffect` variant;
- `BRIDGE_SCHEMA_VERSION` or anything in `bekoedit-ui-contract/src/source_editor.rs`;
- the JavaScript adapter (`js/src/*`) or the committed bundles;
- protected-command semantics, snapshot barriers, deadlines, or takeover;
- `bekoedit-core`, `bekoedit-fs`, `bekoedit-markdown` — this slice is shell-only;
- tree keyboard navigation, `aria-selected`, `aria-disabled` (slice 2);
- menu arrow-key navigation (slice 3);
- any visual redesign.

## 7. Required implementation

### 7.1 Explicit authority state in the controller

Add to `SourceSyncState` a private shell-authority flag with typed accessors —
not a public boolean field:

```rust
pub fn acquire_shell_focus(&mut self) -> Option<u64>;  // cancels pending focus intents, returns guard token
pub fn release_shell_focus(&mut self);
pub fn shell_focus_held(&self) -> bool;
```

`acquire_shell_focus` performs what `cancel_focus_interactions` does today and
additionally sets the flag. While the flag is set, any submitted **focus**
interaction is cancelled immediately instead of being recorded as pending.

**Constraint:** the flag gates focus intents only. It must not gate, defer, or
reject lifecycle operations, protected commands, snapshots, or mounts. A
protected command submitted while a menu is open must still execute normally.
If you find yourself gating anything other than focus, stop and escalate.

### 7.2 Shell-side focus helper

New module `crates/bekoedit-app/src/shell_focus.rs`:

```rust
pub const TRIGGER_APP_MENU: &str = "app-menu-trigger";
pub const TRIGGER_EDITOR_TOOLS: &str = "editor-tools-trigger";
pub const TRIGGER_WORKSPACE_SEARCH: &str = "workspace-search-trigger";
pub const TRIGGER_NEW_FILE: &str = "workspace-new-file-trigger";

/// Focus an element by id on the next animation frame.
pub fn focus_element(id: &str);
```

`focus_element` reuses the existing `requestAnimationFrame` + `getElementById`
pattern already proven at `components/explorer.rs:77`. Centralise it; do not
scatter new `document::eval` strings through components.

**Use static trigger ids and restore directly. Do not build a focus-return
stack.** Shell surfaces are mutually exclusive today — the existing code
already closes the other panels when one opens — so a stack would add state
with no case that exercises it.

### 7.3 Acquire at every open

Every focus-owning surface calls `cancel_source_focus(source_sync)` (which must
now route through `acquire_shell_focus`) **before** moving focus, at the four
gap sites in §3. Add the stable trigger ids from §7.2 to those trigger buttons.

### 7.4 Release and restore at every close

On close, each surface calls `release_shell_focus` and then
`focus_element(<its trigger id>)`. This applies to every close path, including
Escape, the close button, outside-click, and selecting a menu item.

Two rules to hold:

- The shell never focuses CodeMirror directly. If an action needs the editor
  focused afterwards, it goes through the existing controller focus path.
- Restoring focus to a trigger is not a source-focus event and must not
  schedule one.

### 7.5 Settings and Recovery

Settings entry acquires authority; Settings exit releases it and restores focus
to the control that opened Settings. Recovery's own metadata and focus entry
are slice 4 — in this slice, only ensure Recovery does not leave shell authority
held when it unmounts.

## 8. Required tests

**Controller tests** (`source_sync/controller/tests.rs`):

1. `acquire_shell_focus` cancels a pending focus interaction and returns its
   guard token.
2. A focus interaction submitted while shell authority is held is cancelled,
   not recorded as pending.
3. `release_shell_focus` restores normal focus-intent handling.
4. A protected command submitted while shell authority is held still reaches
   its normal outcome — authority must not gate commands.
5. `shell_focus_held()` is false after a controller shutdown.

**Guard tests** (`crates/bekoedit-app/src/tests.rs`), following the existing
`include_str!` assertion style: each of the four trigger sites contains an
acquire call, and each close path contains a `focus_element` restore.

Do not add SSR, hook, or Playwright harnesses — RFC-026 declined them and this
slice does not reverse that.

## 9. Required documentation updates

- None in `docs/src/` for this slice. Document corrections are their own slice.
- If you introduce user-visible strings, add both EN and JA arms plus `ALL_KEYS`
  entries; the existing parity and plain-language guards apply.

## 10. Acceptance criteria

1. Every focus-owning shell surface acquires authority before moving focus.
2. Every close path releases authority and restores focus to its trigger.
3. No source focus effect can be scheduled while shell authority is held.
4. Protected commands behave identically to before this slice.
5. No change to the lifecycle reducer, protocol version, or JS bundles.
6. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --locked`, and `cargo test -p bekoedit --locked` are green.
7. The blocking Xvfb WebView regression still passes unchanged.
8. All touched files remain under the 500-ELOC CI gate; prefer staying under 300.

## 11. Prohibited shortcuts

- No `setTimeout`/sleep to "let focus settle". A timing workaround here
  reproduces the RFC-041 defect class; if ordering seems to require one, the
  design is wrong — escalate.
- No second focus manager, no focus-return stack, no global focus observer.
- No `.unwrap()` / `.expect()` on element lookups; a missing element is a
  no-op, never a panic.
- Do not widen `SourceSyncState`'s public surface beyond the three accessors.
- Do not disable, skip, or weaken an existing test to make this pass.
- No commits, tags, or pushes.

## 12. Compatibility and security constraints

- Bridge protocol stays at version 2; no payload shape changes.
- No new filesystem, network, or process access. Focus restore is DOM-local.
- Element ids are static constants — never interpolate user input, file names,
  or paths into the eval'd focus script.

## 13. Known risks

| Risk | Mitigation |
|---|---|
| Authority flag is left set when a surface unmounts without its close path running (e.g. workspace close while a menu is open) | Release on unmount too, and cover with controller test 5 |
| Restoring focus to a trigger that no longer exists after a re-render | `getElementById(...)?.focus()` is already null-safe; keep it that way |
| Cancelling focus intents too aggressively, so the editor never regains focus after a menu closes | Controller test 4 plus a manual check: open a menu, pick a mode, confirm the editor focuses |
| Scope creep into slices 2–3 | The non-change scope in §6 is explicit; menu arrow keys are not this slice |

## 14. Required evidence

- Changed-file list.
- Output of the four gates in §10.6, plus the WebView regression result.
- New test names and their results.
- A manual note confirming each of:
  1. **Open a workspace from the Start Screen, open a document, confirm the
     editor takes focus.** This is the check that exercises acquire/release
     balance end to end — added 2026-07-31 after slice-1 review finding C1,
     which none of the checks originally listed here would have caught.
  2. Save As on an untitled document, then confirm the editor still takes
     focus (finding C2).
  3. Open a menu, click into the editor: the menu closes and focus **stays**
     in the editor (finding C3).
  4. Open app menu → Escape → focus returns to the trigger.
  5. Open menu → choose a mode → editor becomes focused.

## 15. Required review-request format

Write to `.git-exclude/review-request/2026-07-31-rfc-042-slice-1-focus-authority.md`
with the sections required by the workflow policy §9.2: implementation summary,
addressed requirements, changed files, important implementation decisions,
differences from the approved design, executed tests, test results, build and
static-analysis results, unresolved issues, known limitations, requested review
focus.

Call out explicitly any place where you had to touch RFC-041-owned code beyond
the three accessors in §7.1.
