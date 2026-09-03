# RFC-043 implementation handoff — reopen last workspace on launch

**Governing RFC:** [RFC-043](../../done/RFC-043-reopen-last-workspace-on-launch.md)
**Status:** inherited from RFC-043 (Implemented — merged 2026-09-03, `2afe1df`).
Historical execution guide.
**Baseline:** `main` at `3b4aa40` (refreshed 2026-08-24; originally written
against `91760c7`). The four code references in §1 were re-verified against this
baseline on 2026-08-24 and all still hold exactly — `settings.rs:23`, `:39`,
`settings_screen.rs:89`, `:90`, and `reopen_last_workspace` is still read by no
other code.
**Date:** 2026-08-12

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a mechanism you may replace with something better, without asking —
state what you did and why.

## 1. Purpose, and why it is worth more than it looks

`reopen_last_workspace` is declared at `settings.rs:23`, defaults to `true` at
`:39`, is read and written by a Settings checkbox at `settings_screen.rs:89–90`,
and is consulted by **no other code**. A user toggles it, it persists, and
nothing happens.

Fixing that is the visible half. The other half is that this is the **enabling
change for behavioural test coverage of the whole shell.**

Because `rfd` uses `xdg-portal`, any test that opens a workspace through the
native picker escapes display isolation over D-Bus and renders on the
developer's real session. That is why the WebView regression has never reached
the Explorer, the menus, Settings, Recovery, or a conflict — and why every
keyboard and accessibility contract in RFC-042's five slices is verified by
inspection and guard tests rather than by execution.

A launch-time open driven by settings plus a seeded recents file needs no
dialog. It is the missing piece. **§5.4 is where that value actually lands**;
please read it as load-bearing rather than incidental.

## 2. Current shape of the launch path

Verified on `91760c7`:

| Location | What happens |
|---|---|
| `app.rs:56` | `let settings = persistence.load_settings();` |
| `app.rs:60` | `create_app_state(&persistence)` builds `AppState` |
| `app.rs:73` | `recovery_pending_at_launch` computed from the built state |
| `app.rs:247–254` | routing: `settings_open` → `has_recovery` → `workspace_open` → `StartScreen` |

`workspace_open` is `state.workspace.is_some() || state.session.is_some()`.
`should_show_recovery` additionally requires `session.is_none()`.

Settings are already loaded before state construction, so the decision has a
natural home there — before first render.

## 3. Required implementation

### 3.1 The decision is a pure function · **[Binding]**

```
(setting_enabled, recents, path_is_usable) -> Open(path) | ShowStartScreen
```

Testable with no filesystem and no display. Do not test it by mutating
`HOME`/`XDG_CONFIG_HOME`; inject the inputs.

### 3.2 Recovery keeps precedence — and the mechanism that guarantees it · **[Binding]**

`should_show_recovery` requires `session.is_none()`. Opening a **workspace**
leaves `session` as `None`, so Recovery still wins.

That guarantee holds only because RFC-043 §5 forbids opening a *document*.
**Do not open a document at launch under any circumstance.** If it starts to
look necessary, stop and report — you have found a design problem, not an
implementation detail.

### 3.3 No Start Screen flash · **[Binding]**

The workspace must be open before first render, not applied by an effect
afterwards. A visible Start-Screen-then-MainShell transition on every launch is
a regression in perceived quality.

`create_app_state` is the obvious seam given §2, but the mechanism is
**[Advisory]** — any approach satisfying this and §3.2 is fine.

### 3.4 It must work for `Isolated` persistence, not only `PlatformDefault` · **[Binding]**

`AppPersistence` has two variants. `Isolated(paths)` is what `--webview-smoke`
uses for its disposable profile.

**Both must honour the setting.** If reopen only works for `PlatformDefault`,
the feature ships and the testability benefit in §1 does not — a seeded isolated
profile would still land on the Start Screen, and the driver still could not
reach the shell.

This is the single most consequential line in this handoff. Everything else is
a small feature; this is what makes it the change five slices have been waiting
on.

### 3.5 Failure handling · **[Binding]**

- Validate before opening: the path must exist and be a directory. Use the
  existing `AppState::open_workspace`, which returns `Result` — do not write a
  parallel check.
- Never panic, never block startup, never leave a half-open workspace.
- On failure: fall through to Start Screen with a non-blocking notice naming the
  workspace that could not be opened.
- **Consult only the most recent entry.** Do not try successive recents until
  one works. Silently opening a *different* project than the user last used is
  worse than showing the Start Screen.
- **Do not call `RecentWorkspaces::prune_missing`.** An unmounted volume is
  normal and recoverable; the entry may work tomorrow. Removing it as a side
  effect of a failed launch destroys information the user did not ask to lose.

## 4. Non-change scope · **[Binding]**

- **Reopening the last document, or any session state** — open tabs, scroll
  position, editor mode. §5 of the RFC; §3.2 depends on it.
- The setting's default (`true`) or its Settings-screen presentation. RFC-043 §5
  makes both non-goals. *(RFC-043 §13 Q1 — whether `true` is right for users
  upgrading from versions where the setting did nothing — remains open and is
  the owner's; it does not block this work.)*
- Recovery logic, `should_show_recovery`, or the routing order itself.
- `RecentWorkspaces`' ordering, pruning, or persistence.
- The settings layer refactored by task 005.
- `bekoedit-core`, `-fs`, `-markdown`, `-ui-contract`.
- The WebView driver. Extending it to *use* this is separate work with its own
  design pass; this handoff only makes it possible.

## 5. Required tests · **[Binding coverage, Advisory organization]**

Pure cases, per RFC-043 §10:

1. Setting disabled, recents present → Start Screen.
2. Enabled, recents empty → Start Screen.
3. Enabled, most-recent usable → open it.
4. Enabled, most-recent unusable → Start Screen.
5. **Enabled, most-recent unusable but an older entry usable → Start Screen**,
   not the older entry. This is §3.5's rule and the one most likely to be got
   wrong by a helpful implementation.

Integration-level: a launch with recovery snapshots pending still renders
Recovery, not MainShell.

Prove non-vacuous per standing practice, and where an assertion has more than
one conjunct, prove each independently — the standard from the task-005
correction.

No new GUI harness. Per RFC-042 §10, anything launching a window runs in CI or
on a dedicated display, never the owner's session.

## 6. Acceptance criteria · **[Binding]**

1. Enabled with a usable recent → workspace open at first render, no flash, **no
   document open**.
2. Disabled → launch behaviour byte-identical to today.
3. Pending recovery still renders Recovery.
4. Unusable path → Start Screen, visible notice, no panic, recents unmutated.
5. Works for both `PlatformDefault` and `Isolated`.
6. Decision covered by pure tests needing no display.
7. The Settings checkbox now controls observable behaviour.
8. Pinned gates green (`+1.88.0`); CI green including the WebView regression and
   the eval-script parse-check.
9. Every file under 500 ELOC.

## 7. Prohibited shortcuts · **[Binding]**

- No document restore.
- No falling through to older recents.
- No `prune_missing` on failure.
- No effect-based open that flashes the Start Screen first.
- No `unwrap`/`expect` on path resolution.
- No `--force` push.

## 8. Required evidence

- Changed-file list; before/after ELOC.
- The pure test names and results, plus non-vacuity evidence.
- **Explicit confirmation that `Isolated` honours the setting** (§3.4), and how
  you verified it.
- Confirmation that no document is opened at launch (§3.2).
- Pinned gate output and the CI run result.
- A manual note if a safe display is available; if not, say so plainly.

## 9. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL. Merging,
merge mechanism, tags, and releases require explicit instruction. If `main` has
moved, report the topology and stop.

Commit scope `app:`; reference RFC-043.

## 10. Review-request format

`.git-exclude/review-request/<date>-rfc-043-reopen-last-workspace.md`, workflow
policy §9.2 sections. Lead with the §8 `Isolated` confirmation.

## 11. What follows this

RFC-043 moves to `rfcs/done/` when this merges — my action, not yours; do not
touch the RFC.

It is also the last approved RFC in the portfolio. Once it lands, the workflow
policy requires me to report the planning state to the project owner and reopen
roadmap discussion rather than start another theme.
