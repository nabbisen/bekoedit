# RFC-042 handoff — slice 4: conflict, recovery, and settings

**Governing RFC:** [RFC-042](../../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) §7.5, §7.6, §8
**Slice:** 4 of 7 — the last slice in v0.14.0 scope
**Depends on:** slices 1–3
**Status:** inherited from RFC-042 (Implemented). Historical execution guide — see RFC-042 §13.1 for this slice's disposition.
**Date:** 2026-08-04

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**, per the convention from
slice 2. Binding is mine to decide; advisory is a suggestion you may replace
with reasoning, without asking.

## 1. Purpose

Three surfaces currently expose no accessibility metadata at all (RFC-042 F-6,
F-7): the conflict banner, the Recovery screen, and the Settings screen. Two of
them are data-safety surfaces. This slice gives them roles, names,
announcements, and — for the two screen replacements only — focus management.

## 2. Read §7.6 of the RFC before starting · **[Binding]**

**It was amended on 2026-08-04 and now says the opposite of what it said when
this theme was approved.** The original required the conflict banner to move
focus to its first action. That was my error and it was a data-loss hazard:
action 1 overwrites the on-disk version, action 2 discards all unsaved local
edits, and the banner appears in response to an external event that can arrive
mid-keystroke.

If you have read an older copy of RFC-042, re-read §7.6.

## 3. Applicable RFC and requirements

- RFC-042 §7.5 (screen replacements), §7.6 (conflict banner, **amended**), §8
  (accessibility metadata), §5 (declare-or-implement), §6.2 (focus authority)
- RFC-042 F-6, F-7
- RFC-000 §11; RFC-023 (error surfaces); product principle 6.7

## 4. Change scope · **[Advisory]**

- `crates/bekoedit-app/src/components/conflict_banner.rs`
- `crates/bekoedit-app/src/components/recovery_screen.rs`
- `crates/bekoedit-app/src/components/settings_screen.rs`
- `crates/bekoedit-app/src/i18n.rs` — new keys, EN and JA
- `crates/bekoedit-app/src/tests.rs` — guard assertions
- `crates/bekoedit-app/assets/style.css` — `:focus-visible` only if needed

## 5. Required implementation

### 5.1 Conflict banner — announce, never seize · **[Binding]**

- `role="alert"` on the banner, with an accessible name from a translated key.
- **No focus movement of any kind.** Not on appear, not on resolve, not
  deferred, not conditional.
- **No IME-composition guard.** There is no focus move to defer; adding one
  would imply a focus move exists.
- The three action buttons stay reachable by Tab in document order. Do not add
  `tabindex`, do not reorder them, do not change what they do.

If you believe the banner needs focus for the decision to be discoverable, stop
and raise it — do not implement a compromise.

### 5.2 Recovery screen · **[Binding]**

A screen replacement (§7.5), and the surface that offers unsaved work back
after a crash.

- Landmark role and an accessible name.
- Focus moves to its heading on entry.
- Focus returns to the invoking control on exit — but Recovery is entered at
  launch, not from a control, so on exit restore focus to whatever the next
  screen's natural first control is rather than inventing a phantom trigger.
- **A live region announcing how many documents are recoverable.** A user who
  cannot see the screen must learn that unsaved work is on offer; this is the
  single most important item in this slice.
- Do not trap focus. This replaces the shell rather than layering over it.

### 5.3 Settings screen · **[Binding]**

- Landmark role and accessible name; focus to its heading on entry.
- On exit, restore focus to the app-menu trigger, as slice 1 established.
- Slice 1 left a comment recording that any future Settings exit path must also
  release shell focus authority. If you add one — Escape, for instance — it must
  call the same `close_settings` path. If you add no new exit, change nothing
  there.

### 5.4 Metadata across all three · **[Binding]**

Per §8: every interactive control has an accessible name from a translated key;
icon-only controls carry both `aria-label` and `title`; state conveyed by colour
is also conveyed by text or ARIA. No literal user-facing strings in RSX.

### 5.5 Focus authority · **[Binding]**

Settings and Recovery are focus-owning surfaces under §6.3, so they acquire on
entry and release on exit through the slice-1 accessors. Consume them; do not
extend them.

The conflict banner is **not** a focus-owning surface — it takes no focus, so it
neither acquires nor releases.

## 6. Non-change scope · **[Binding]**

- Conflict *semantics*: what the actions do, their order, when the banner
  appears, `ConflictState`, or anything in `bekoedit-core`. This slice changes
  presentation only.
- Recovery *logic*: `recovery.list()`, snapshot restore, routing precedence in
  `app.rs`.
- Settings *content*: which settings exist, defaults, persistence.
- Slices 1–3 surfaces: tree, menus, tabs, `shell_focus` helpers.
- `source_sync/` in any form.
- `bekoedit-core`, `-fs`, `-markdown`, `-ui-contract`.

## 7. Required tests · **[Binding coverage, Advisory organization]**

This slice is mostly static metadata, which guard assertions genuinely do
verify — a `role` attribute either appears in the source or does not. That makes
it better covered than slice 3 was, not worse.

1. **Guard assertions**: `role="alert"` and an accessible name on the banner;
   landmark roles and names on both screens; the Recovery live region exists.
2. **A negative assertion that the conflict banner performs no focus call** —
   it contains no `shell_focus::`, no `focus_element`, no `.focus()`. This is
   the §5.1 regression guard and the most valuable assertion in the slice.
3. **i18n**: new keys in `ALL_KEYS` with both arms; existing parity and
   plain-language guards apply.
4. Any new eval script must be covered by the balance test added in slice 3's
   correction. Prefer adding no new eval script.

## 8. Documentation · **[Binding]**

No `mvp-acceptance.md` change — no acceptance item covers these surfaces.

If this slice completes v0.14.0's scope, say so in the review request; do not
update `ROADMAP.md` or `CHANGELOG.md` yourself. Release framing is mine.

## 9. Acceptance criteria · **[Binding]**

1. The conflict banner announces via `role="alert"` and moves no focus.
2. Recovery and Settings expose landmark roles, names, and focus on entry/exit.
3. Recovery announces the recoverable-document count via a live region.
4. All new strings are translated in both languages.
5. No conflict, recovery, or settings *behaviour* changed.
6. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo test -p bekoedit --locked` green.
7. The blocking Xvfb WebView regression passes in CI.
8. Every file under 500 ELOC. `tests.rs` is at 394 and grows again here — if it
   passes ~450, split it rather than letting it approach the gate.

## 10. Prohibited shortcuts · **[Binding]**

- No focus movement on the conflict banner, in any form.
- No focus trap on either screen.
- No `setTimeout`/sleep to sequence focus.
- No changes to conflict, recovery, or settings behaviour.
- No editing slice 1–3 focus helpers.
- No weakening or deleting an existing test.
- No `--force` push. The slice-3 exception was scoped to that branch and does
  not carry forward.

## 11. Known risks · **[Advisory]**

| Risk | Mitigation |
|---|---|
| Focus-on-appear reintroduced for the banner because it "feels" more accessible | §7.2's negative assertion |
| Recovery's live region announces on every render rather than on entry | Announce the count once; verify the region is not re-populated per render |
| Restoring focus on Recovery exit finds nothing to restore to | §5.2 — target the next screen's first control, not a phantom trigger |
| `tests.rs` growth | §9.8 |

## 12. Required evidence

- Changed-file list; before/after ELOC for each touched file.
- Gate output per §9.6, plus the CI WebView regression result.
- **Explicit confirmation that the conflict banner performs no focus call**,
  and how you verified it.
- Confirmation that no conflict/recovery/settings behaviour changed.
- A manual note if a safe display is available; if not, say so plainly rather
  than omitting it — do not drive the owner's session (RFC-042 §10).

## 13. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL and result.
Merging, merge mechanism, tags, and releases require explicit instruction.

If `main` has moved by then, report the topology and stop rather than choosing a
merge strategy yourself.

Commit scope `app:`; reference RFC-042 slice 4.

## 14. Review-request format

`.git-exclude/review-request/<date>-rfc-042-slice-4-conflict-recovery-settings.md`,
with the workflow policy §9.2 sections. Lead with the §12 conflict-banner
confirmation.
