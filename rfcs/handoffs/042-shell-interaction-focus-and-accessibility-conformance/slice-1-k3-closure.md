# RFC-042 slice 1 — K3 closure: instruction to the dev team

**Governing RFC:** RFC-042 · **Slice:** 1 (implementation already approved)
**Supersedes:** the "no commits, tags, or pushes" clause of
`slice-1-focus-authority.md` §11, **for this task only**
**Date:** 2026-07-31

---

## 0. Authorization

The project owner has authorized pushing for this change. §11's prohibition is
lifted for the specific actions below and nothing else — no tags, no release
actions, no force-push, no history rewriting.

The owner also corrected my routing of the verification work: **testing is your
role.** I conflated "the dev team has no safe display" with "this belongs to
the owner." Those are different problems, and §3 solves the first one.

## 1. Commit and push

Branch: `rfc-042-slice-1-focus-authority` (branch from `main`; do not commit to
`main` — the entire point of the WebView gate is that it runs *before* this
reaches the main line).

Two commits, split by path. The split is clean — no hunk surgery needed:

**Commit A — governance and documentation**

```
rfcs/**
docs/src/mvp-acceptance.md
ROADMAP.md
```

```
docs: add RFC-042 shell conformance, slice handoffs, RFC-031 disposition

Adds RFC-042 (shell interaction, focus, and accessibility conformance)
and its slice 1/2 handoffs. Moves RFC-031 to done/ as a reached decision.
Corrects the mvp-acceptance file-tree item, whose cited aria-selected
evidence did not exist, and the ROADMAP claim that RFC-010 shipped
resizable sidebar panes.
```

**Commit B — code**

Everything else, including `CHANGELOG.md` and `assets/style.css`.

```
app: establish single shell/source focus authority (RFC-042 slice 1)

Adds explicit shell focus authority to the source-sync controller, so the
shell and the RFC-041 controller can no longer both move focus. Shell
surfaces acquire before moving focus and release on close; the four
authority-holding surfaces are mutually exclusive.

Includes pre-existing v0.14.0 explorer/menu accessibility work that shares
the same files and cannot be separated by path.

Note: explorer.rs still carries `disabled: !is_openable` and a test
asserting it. RFC-042 §11 declares this non-conformant; slice 2 §7.4
reverts it. Committed knowingly, not as intended behavior.
```

That last paragraph is required. Without it, history records a known-wrong
attribute as deliberate, and a future reader finds a test mandating it.

Push the branch. Do not merge. Report the CI run result back for review.

## 2. WebView regression via CI

Pushing the branch triggers `.github/workflows/ci.yml`, including the blocking
"WebView lifecycle regression (RFC-041)" job. That closes acceptance criterion
§10.7 for slice 1 without touching any local display.

Report the run URL and result.

## 3. The Start Screen check — restructure it, do not drive the owner's session

This is the part I got wrong, and there is a technical reason it matters beyond
role assignment.

**`xvfb-run` alone would not have made this safe.** `rfd` is built with the
`xdg-portal` feature (`Cargo.toml:36`). The folder picker is not drawn by the
application — it is a D-Bus request to `org.freedesktop.portal.Desktop`, which
is live on the owner's user session bus (verified: `xdg-desktop-portal`,
`-gnome`, and `-gtk` backends all running under `user@1000.service`). The
portal backend renders wherever *it* is bound, which is the owner's real
compositor. Xvfb isolates the app's own windows; the portal dialog escapes that
isolation over D-Bus and appears on the owner's screen regardless.

So "run it under Xvfb" would not have been the safe answer I implied.

**Restructure the check instead.** What C1 needs verified end-to-end is: *after
a workspace opens, does the editor take focus?* The folder picker is incidental
— it is merely how the workspace path is chosen. Two paths reach the same state
without any dialog:

- the **recent-workspaces list** on the Start Screen
  (`start_screen.rs`, `start-recent-btn`), which dispatches an open directly;
- a pre-seeded workspace path in a disposable profile.

Either exercises the acquire/release balance through the identical code path.
Neither touches the portal. Both are safe on an isolated display.

**Required.** Extend the existing `--webview-smoke` driver with a check that
opens a workspace by a non-dialog path, opens a document, and asserts the
editor holds focus afterwards. `webview_smoke` contains no `rfd` usage today —
verified — so it stays portal-free and CI-safe. This converts the one check
that "needed a human" into a regression that runs on every push, which is worth
more than a one-time observation.

If you find a reason this cannot be automated, say so and stop — do not fall
back to driving the live session.

## 4. Standing constraint

Never launch a GUI process or send synthetic input against the owner's live
desktop session. Session-scoped input tools and D-Bus portals both ignore
display isolation. RFC-042 §10 records this as policy for all slices.

## 5. Out of scope

- Merging to `main` — report CI results first.
- Slice 2 — still blocked until slice 1 merges.
- Installing system packages. If §3 turns out to need one, ask rather than
  install.
