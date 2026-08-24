# RFC-043: Reopen Last Workspace on Launch

**Project:** bekoedit
**Status:** Accepted — approved for implementation by the project owner
2026-08-04. Not yet built. Moved from `proposed/` to `accepted/` on 2026-08-24
when this project adopted the 5-folder variant, which gives "the owner approved
this design" a folder of its own; the qualifier this line used to carry is now
the folder's job.
**Handoff:** [`handoffs/043-reopen-last-workspace-on-launch/`](../handoffs/043-reopen-last-workspace-on-launch/implementation-handoff.md)
**Track:** Workspace lifecycle
**Priority:** Medium
**Date:** 2026-08-04
**Related RFCs:** [RFC-003](../done/RFC-003-workspace-model-and-recent-workspaces.md), [RFC-007](../done/RFC-007-save-autosave-atomic-write-and-recovery.md), [RFC-022](../done/RFC-022-settings-preferences-and-local-configuration.md), [RFC-042](../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md)

---

## 1. Summary

Implement the `reopen_last_workspace` setting, which currently exists, defaults
to `true`, is presented to users as a Settings checkbox, and is read by no code
at all.

On launch, when the setting is enabled and a usable recent workspace exists,
open that workspace before the first render — without opening any document, and
without displacing crash recovery.

## 2. Current state

Verified 2026-08-04:

| Location | What it does |
|---|---|
| `bekoedit-app/src/settings.rs:23` | declares `reopen_last_workspace: bool` |
| `bekoedit-app/src/settings.rs:39` | defaults it to `true` |
| `bekoedit-app/src/components/settings_screen.rs:72–73` | reads and writes it via a checkbox |
| anywhere else | nothing |

The value is persisted and honored by no behavior. A user toggles the control,
the setting saves, and the application does the same thing either way.

This is a control that makes a promise the application does not keep. The
default of `true` records the intended behavior as reopen-on-launch, so the
feature was designed and never wired up — not decided against.

## 3. Motivation

Three reasons, in order of weight:

1. **The Settings screen is currently misleading.** RFC-042 exists to make the
   shell tell the truth about itself; an inert control is the same defect class
   as a `col-resize` cursor on a divider that does not drag.
2. **The behavior is expected.** Reopening the last project is standard for
   local-first editors, and the recorded default says this project intended it.
3. **It creates a dialog-free workspace-open path at launch.** This is the
   piece the declined RFC-042 smoke-driver extension lacked. Because `rfd`
   uses `xdg-portal`, any test that opens a workspace through the native picker
   escapes display isolation via D-Bus and renders on the developer's real
   session. A launch-time open driven by settings plus a seeded recents file
   needs no dialog, so the Start Screen focus check becomes automatable later
   without touching a portal.

## 4. Goals

- Honor `reopen_last_workspace` on launch.
- Preserve crash-recovery precedence exactly as it is today.
- Fail safe and visibly when the recorded path is no longer usable.
- Keep the decision logic pure and headlessly testable.

## 5. Non-goals

- **Reopening the last *document*.** Only the workspace is restored. Document
  restore interacts with dirty state, recovery snapshots, and conflict
  detection, and is a separate design.
- Session restore of any kind — open tabs, scroll position, editor mode.
- Multi-root or multi-workspace behavior; single-root stands (NG per RFC-003).
- Changing the setting's default or its Settings-screen presentation.
- Pruning or reordering recents beyond what §8 requires.

## 6. User-facing behavior

On launch:

- **Setting enabled, a usable recent workspace exists** — that workspace is
  open when the window first appears. The file tree is populated. No document
  is open, so the editor pane shows its existing empty-state hint.
- **Setting enabled, no recents** — Start Screen, unchanged.
- **Setting enabled, most recent path unusable** — Start Screen, with a
  non-blocking notice naming the workspace that could not be opened. The
  recents list still shows it.
- **Setting disabled** — Start Screen, unchanged.
- **Crash recovery pending** — the Recovery screen shows, exactly as today.
  See §7.

## 7. Launch routing and precedence

This is the part requiring care. `app.rs` routes as:

```
settings_open → SettingsScreen
has_recovery  → RecoveryScreen
workspace_open → MainShell
otherwise     → StartScreen
```

where `workspace_open` is `workspace.is_some() || session.is_some()`, and
`should_show_recovery` additionally requires `session.is_none()` and a
pending-at-launch snapshot.

**Binding requirement: recovery must continue to take precedence over an
auto-opened workspace.** This holds because recovery's guard tests
`session.is_none()`, and §5 forbids opening a document — opening a *workspace*
leaves `session` as `None`. Any implementation that opens a document at launch
would break this and is out of scope.

**Binding requirement: no Start Screen flash.** The workspace must be open
before first render, not applied by an effect afterwards. A visible transition
from Start Screen to MainShell on every launch is a regression in perceived
quality.

Mechanism is advisory: constructing the state with the workspace already open
is the obvious route, but the implementer may choose otherwise provided both
requirements above hold.

## 8. Failure handling

The recorded path may be deleted, renamed, or on an unmounted volume.

- Validate before opening: the path must exist and be a directory. Reuse the
  existing workspace-open path (`AppState::open_workspace`, which returns
  `Result`) rather than a parallel check.
- A failure must never panic, block startup, or leave a partially-open
  workspace.
- On failure, fall through to Start Screen and surface a non-blocking notice.
- **Do not silently drop the failing entry from recents.** An unmounted volume
  is a normal, recoverable condition and the entry may work tomorrow.
  `RecentWorkspaces::prune_missing` exists and must not be invoked as a side
  effect of this feature.

## 9. Security and safety

- The path is read from persisted local settings, which is user-owned data —
  not untrusted input. It is validated as an existing directory before use.
- Workspace-root confinement (SEC-004) applies unchanged: the opened root
  becomes the confinement boundary exactly as when opened by dialog.
- No new filesystem authority, no network, no new process execution.
- Only one path — the most recent entry — is consulted. The feature does not
  iterate recents looking for one that works, which would turn a missing volume
  into a silent switch to a different project.

## 10. Testing strategy

The decision is a pure function and must be tested as one, headlessly:

```
(setting_enabled, recents, path_is_usable) -> Open(path) | ShowStartScreen
```

Required cases: disabled setting with recents present; enabled with empty
recents; enabled with a usable most-recent entry; enabled with an unusable
most-recent entry; enabled where the most recent is unusable but an older entry
is usable (must **not** fall through to the older one, per §9).

Integration-level: a launch with recovery snapshots pending must still render
Recovery, not MainShell.

No new GUI harness. Per RFC-042 §10, any check launching a window runs in CI or
on a dedicated display.

## 11. Acceptance criteria

1. With the setting enabled and a usable recent, the workspace is open at first
   render with no Start Screen flash and no document open.
2. With the setting disabled, launch behavior is byte-identical to today.
3. Pending crash recovery still renders the Recovery screen.
4. An unusable recorded path yields Start Screen plus a visible notice, no
   panic, and no mutation of the recents list.
5. Decision logic is covered by pure tests requiring no display.
6. The Settings checkbox now controls observable behavior.

## 12. Alternatives considered

**Remove the setting and the checkbox instead.** Smaller, and honest
immediately. Rejected: the `true` default records the intended behavior, so
deletion would ratify "we never reopen" as a decision that was never actually
made — and it would discard the testability benefit in §3.3.

**Also reopen the last document.** Rejected for this RFC: it interacts with
recovery precedence, dirty state, and conflict detection at exactly the moment
those are least observable. It is a coherent follow-up once this lands.

**Change the default to `false`.** Rejected: it would alter behavior recorded
by an existing persisted setting, surprising anyone who has it enabled today.
See §13.

**Try successive recents until one opens.** Rejected in §9: silently opening a
different project than the user last used is worse than showing Start Screen.

## 13. Open questions

1. Should the default remain `true` for users upgrading from a version where
   the setting did nothing? Enabling real behavior on a setting they never
   meaningfully chose is a mild surprise. This is the same shape as RFC-041
   §10's upgrader question, and I suggest resolving both together rather than
   inventing a per-setting policy.
2. Should the failure notice offer a "remove from recents" action, or stay
   purely informational? Informational is proposed; an action turns a notice
   into a decision surface with its own accessibility contract (RFC-042 §5).
