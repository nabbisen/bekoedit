# RFC-047: User commands during source-editor transitions

**Project:** bekoedit
**Status:** Implemented — on `main`, not yet released. Accepted and drafted the
same day, 2026-09-22, after RFC-044 slice 3 found the behaviour and task 021
worked around it in the harness.

- **Slice 1** (`4476f90`): the queue. A command that would have been answered
  `Busy` is held, coalesced and expired, and every removal is recorded. It also
  unified the pre-existing depth-1 `waiting_command` slot into the same queue, so
  one mechanism remains.
- **Task 022** (`7a0ca07`), which slice 1 uncovered: a switch claims editor focus
  against the controller's effective target, not the UI mode, so the focus layer
  and the controller share one notion of where the app is heading.
- **Slice 2** (`79b45ec`): the report. One plain sentence per discarded command,
  grouped by reason, with word order translated rather than assembled in English;
  the last silent drop (`relay_disconnected`) closed; and RFC-044's
  `queued_switch_claims_focus` phase, whose mutation fails naming itself.

*Moved to `done/` 2026-09-22.* Nothing is deferred. The behaviour recorded for
the owner in slice 1's review — a mode click lost during a teardown — is what
this RFC removed.
**Track:** Editor lifecycle
**Priority:** Medium — no data is lost today, but a user action can vanish with no
trace a user can see
**Date:** 2026-09-22
**Related RFCs:** [RFC-041](../done/RFC-041-source-editor-lifecycle-and-synchronization-controller.md) (the lifecycle this changes), [RFC-042](../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) (focus authority), [RFC-044](../done/RFC-044-shell-behaviour-regression-coverage.md) (found it)

---

## 1. Summary

While the source editor is in a lifecycle transition, the controller answers
`Busy` to a user command, and the caller drops it after writing one trace line.
Nothing runs, and nothing tells the user.

This RFC makes one rule: **a command a user asked for is either executed, or the
user is told it was not.** It is delivered by accepting the command into a small
bounded queue that drains when the transition ends, and by reporting the cases
where the queue cannot honour it.

## 2. Motivation

`SourceController::submit_with_focus` handles the states it knows and ends with
`_ => SubmitOutcome::Busy` (`source_sync/controller.rs`).
`submit_source_command_preserving_focus` (`source_sync.rs:93-95`) handles `Busy`
with `bridge::trace(…)`, which writes to stderr only when `BEKOEDIT_SOURCE_TRACE`
is set. In a release build launched from a desktop there is no console at all
(task 018).

Three user actions reach that path, in rising order of seriousness:

1. **A mode switch.** Click Preview, then Text before the Text editor has finished
   tearing down: the second click does nothing. This is how RFC-044's D2 check
   failed at random, which is how we found it.
2. **Opening a document.** Enter on a tree row, a search result, or a backlink
   during a transition: the file does not open. The user believes they opened it.
3. **An explicit save.** `Ctrl+S` is `SourceCommand::SaveNow`
   (`app.rs:198-204`). Dropped during a transition, the keystroke does nothing.
   Autosave still runs and the status bar still reads dirty, so no work is lost —
   but the one gesture a user makes when they want certainty is the one that
   silently did nothing.

The window is short: an IPC hop plus a render, a few milliseconds in practice. It
is not zero, it grows on a loaded machine, and it is reachable by ordinary
double-clicking.

## 3. Goals

- No user-initiated command is lost without being executed or reported.
- The common case stays invisible: a queued command that runs a few milliseconds
  later needs no message and no spinner.
- Bounded in time and in memory, with no new way to wedge the editor.
- The queue never executes a command whose meaning has changed.

## 4. Non-goals

- **Changing the barrier model.** RFC-041's snapshot-and-hold is unchanged. This
  RFC queues *before* `submit`, and each queued command is submitted normally.
- **Retrying failures.** A command that runs and fails is unchanged.
- **Queuing editor input.** Typing is not a `SourceCommand`; CodeMirror owns it.
- **`Unavailable`.** That already raises an error toast and waiting does not fix
  it (§5.6).
- **A progress indicator.** §5.5 explains why, with a trigger to revisit.

## 5. Design

### 5.1 The rule · **[Binding]**

When `submit` would answer `Busy`, the command is **accepted into a queue**
instead of dropped. The queue drains when the controller settles. Every path out
of the queue is one of: executed, superseded by the user's own newer command, or
**reported to the user**.

### 5.2 What is queued, and how many · **[Binding]**

- **Depth 2, FIFO.** Two covers the realistic burst — open a file then switch
  mode, or save then switch — and keeps the structure inspectable. Deeper queues
  replay stale intent.
- **Same-kind coalescing.** A queued `SwitchMode` is *replaced* by a newer
  `SwitchMode`: the user changed their mind, and running both would flash an
  intermediate mode. A queued `SaveNow` absorbs another `SaveNow`, which is
  idempotent. Nothing else coalesces, because nothing else is a restatement of
  the same intent.
- **Overflow rejects the newest, and says so.** When the queue is full, the new
  command is not run and the user is told (§5.5). Dropping the *oldest* silently
  to make room would re-create the defect this RFC exists to remove.
- **Order is preserved** for commands that are not coalesced. Opening a document
  and then switching mode must happen in that order.

### 5.3 When it drains, and when it expires · **[Binding]**

The queue is attempted whenever the controller's state changes and on the
existing tick. Each entry carries a deadline of **6 s**, longer than the longest
lifecycle deadline it can wait behind (`MOUNT_DEADLINE_MS` is 5 s).

The deadline exists because one busy state is **not** time-bounded.
`BarrierHeld` has no `PendingOperation`, so `next_deadline` returns `None` for it
(`lifecycle/controller_support.rs`). It is held while `commands::execute` runs
synchronously, which is file I/O: fast on a local disk, arbitrarily slow on a
stalled network mount. A queue that trusted the lifecycle's own deadlines would
wait forever there.

On expiry the entry is discarded and reported (§5.5).

**Unmounting the editor does not empty the queue** *(corrected 2026-09-22, at the
slice 1 review; the handoff had said otherwise)*. Executing a mode switch is
precisely what unmounts the editor: the host's `TextMode` drop calls
`force_unmount`. A queue emptied there would lose "Preview, then Form" the moment
Preview executed, which is the case this RFC exists for. Entries survive the
teardown and run when it ends. Shutdown, relay loss and an editor that has become
`Unavailable` do empty it, and each records what it emptied.

### 5.4 The queue never runs a command whose meaning changed · **[Binding]**

Each entry records the session fingerprint at the moment it was accepted. Before
a **document-scoped** command runs — `SaveNow`, `SaveAs`, `MoveSectionUp`,
`MoveSectionDown`, `RestoreHistory` — the fingerprint's `document_id` must still
match. If the open document changed while the entry waited, the entry is
discarded and reported.

This is the safety rule of the design. Saving document B because the user pressed
`Ctrl+S` while document A was open would be a data-loss defect introduced by a
convenience feature.

`SwitchMode` and `OpenDocument` are not document-scoped: one names a mode, the
other names a path, and both mean the same thing whenever they run.

### 5.5 What the user sees · **[Binding]**

**Nothing, when it works.** The queue exists because transitions take
milliseconds. A toast or a spinner for every queued command would turn an
invisible success into visible noise, and would train users to ignore toasts.

**A message, when it does not.** One `Warning` toast, naming the action in the
user's words — "Could not switch to Form — the editor was busy", never a lifecycle
state name. New i18n keys carry both EN and JA arms, and the existing parity and
plain-language guards apply.

*Amended 2026-09-22, after slice 1.* This said "exactly three cases". Slice 1's
implementation named six reasons a command can leave the queue, so the rule is
stated per reason:

| Reason | Reported |
|---|---|
| Overflow (§5.2), expiry (§5.3), changed document (§5.4) | yes |
| The editor became unavailable, the relay to the page was lost | yes — the command will not run, and waiting will not fix it |
| Shutdown | **no** — the window is closing, and a toast that cannot be read is noise in the log rather than information for a user |

**When several are discarded at once, they are reported together**, grouped by
reason, naming how many. Emptying a queue of two plus any commands already handed
to the host must not produce a stack of toasts for one event.

**Controls are never disabled during a transition.** Disabling a focused mode tab
would move focus out from under the keyboard, and a screen-reader user would hear
a control disappear for reasons they cannot perceive — the same mistake RFC-042
§7.1 corrected for tree rows. The controls stay enabled; the command is accepted.

**No progress indicator in the first release.** `SourceEditorStatus`
(Loading / Ready / Unavailable) already exists for a mounting editor and is
unchanged. Adding a second "working" vocabulary for a 5 ms wait would be worse
than silence. **Revisit trigger:** a measurement showing queued commands commonly
waiting beyond ~300 ms, which is where a person starts to perceive delay.

### 5.6 What is unchanged · **[Binding]**

- `NoOp` stays silent. There is nothing to report.

  *Amended 2026-09-22, by task 022.* This said "switching to the mode already
  shown". That test read only the **mounted** editor, so it called a click a no-op
  while a switch to another mode was already on its way, and the user's last click
  was lost — the same defect this RFC exists to remove, one layer up. A switch is a
  no-op only against the mode the app is **heading for**: never when another switch
  is queued, else the one in flight, else the mounted editor. That single rule is
  `SourceSyncState::is_same_source_mode`, and the focus layer compares against it
  too rather than keeping its own copy.
- `Unavailable { retired: Some(_) }` keeps its error toast. Waiting cannot resolve
  it, so it must not be queued.
- The focus token travels with the queued entry. If the user moved focus while it
  waited, the focus guard has already diverted and the claim is refused: the
  command runs, focus stays where the user put it. That is RFC-042's rule, not an
  exception to it.

## 6. Alternatives considered

**Reject with a toast, and no queue.** Honest and much simpler. Rejected: for a
5 ms transition it converts an invisible success into a visible failure, and it
teaches the user that the app randomly refuses clicks.

**Disable the controls while busy.** Rejected in §5.5. It is the option that looks
most "correct" and behaves worst for keyboard and screen-reader users.

**Queue everything, unbounded.** Rejected: replays stale intent, and `BarrierHeld`
has no deadline to bound it.

**Leave it, and let the harness work around it.** That is today's state, with task
021's settle gate. It keeps the suite green while the product keeps the defect —
precisely the inversion RFC-044 was written to prevent.

**Give `BarrierHeld` a deadline instead.** Rejected: the barrier is held across a
synchronous call, so a deadline cannot interrupt it. §5.3's queue-side deadline is
the honest bound.

## 7. Testing

- **Pure reducer tests**, headless: accept while in each busy state; coalescing;
  FIFO order; overflow rejecting the newest; expiry; the §5.4 fingerprint
  mismatch; and that `NoOp` and `Unavailable` are untouched. Each proven able to
  fail by mutation.
- **A WebView phase in RFC-044's second run.** Click Preview and then Text **in
  the same exchange**, which is exactly the shape that raced before task 021, and
  assert that Text still wins. Because both clicks happen inside one exchange,
  task 021's settle gate cannot mask it, and this phase becomes the executable
  proof that the queue works in the real WebView.
- **i18n guards** for the new keys.

## 8. Interaction with task 021

Task 021's settle gate stays. It is the harness declining to *act* inside a
transition, which keeps D2 measuring authority release rather than queue
behaviour. This RFC makes the app correct when a **user** acts inside one. The two
are complementary, and §7's new phase is what stops the gate from hiding the
product defect.

## 9. Slices

1. **The queue**, in the controller: §5.1–§5.4, §5.6, and §7's reducer tests.
   Headless; no UI change.
   Handoff:
   [`handoffs/047-user-commands-during-editor-transitions/slice-1-the-queue.md`](../handoffs/047-user-commands-during-editor-transitions/slice-1-the-queue.md).
   It also unifies the existing depth-1 `waiting_command` slot into this queue,
   so one mechanism remains rather than two.
2. **The reporting**: §5.5's toast, its i18n keys, and §7's WebView phase.
   Handoff:
   [`handoffs/047-user-commands-during-editor-transitions/slice-2-the-report.md`](../handoffs/047-user-commands-during-editor-transitions/slice-2-the-report.md).
   It also closes the one silent drop slice 1 left: `relay_disconnected` clearing
   an `Execute` the host had not run yet.

## 10. Questions, answered

1. **Depth 2 or 1?** **2.** One slot cannot hold "open this file, then show me
   Form", which is an ordinary two-click sequence. Three or more only stores
   intent old enough to be stale.
2. **Warning or Info for the report?** **Warning.** The user asked for something
   that did not happen. `Info` is for things that did.
3. **Should `SaveNow` be special?** **No, beyond §5.4.** It queues and reports like
   anything else. Autosave and the dirty indicator remain the safety net, and a
   special retry path for one command would be a second mechanism to reason about.
4. **Is the 6 s deadline arbitrary?** It is chosen, not arbitrary: one second past
   the longest lifecycle deadline a queued command can legitimately wait behind. If
   `MOUNT_DEADLINE_MS` changes, this changes with it, and a test pins that
   relationship.
