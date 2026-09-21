# RFC-047 handoff — slice 1: the queue

**Governing RFC:** [RFC-047](../../accepted/RFC-047-user-commands-during-editor-transitions.md) §5.1–§5.4, §5.6, §7
**Slice:** 1 of 2 — the controller. Slice 2 is the user-facing reporting.
**Baseline:** `main` at `5888d49` or later. If `main` moves in a file this slice
touches, merge `origin/main` in; never rebase.
**Status:** inherited from RFC-047 (Accepted 2026-09-22)
**Date:** 2026-09-22

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`.

## 1. Purpose

A command a user asked for is either executed, or reported. Today, one submitted
during a lifecycle transition is answered `Busy` and dropped after a trace line
that a release build writes nowhere a user can see.

This slice makes the controller hold it. **No UI changes here.** Slice 2 turns
the reports into toasts.

## 2. What exists · verified on `5888d49`

**The drop.** `submit_with_focus` (`source_sync/controller.rs:108-159`) ends in
`_ => SubmitOutcome::Busy`. `submit_source_command_preserving_focus`
(`source_sync.rs:88-102`) matches `Busy` and only calls `bridge::trace`.

**A depth-1 queue already exists, for one case.** `queue_for_mount`
(`controller.rs:205-219`) parks a command in `waiting_command` while the
lifecycle is `Mounting` or `Initializing` for the same document, and answers
`Busy` when that single slot is taken. `start_waiting_command`
(`controller.rs:222-233`) drains it, but **only if the state is `Ready`**;
otherwise it drops the command silently. It is called from `EditorReady` and
`InitFailed` (`controller/events.rs:44,50`).

**Four places clear that slot silently today:**

| Site | When |
|---|---|
| `force_unmount` (`controller.rs:61-62`) | the host unmounts the editor |
| `shutdown` (`controller.rs:69-70`) | app shutdown |
| `tick` (`controller.rs:195-197`) | a timeout left the lifecycle `Unavailable` |
| `handle_event` tail (`controller/events.rs:91-94`) | an event left it `Unavailable` |
| `relay_disconnected` (`controller/support.rs:108`) | the relay generation was lost |

**Busy states.** `busy_lifecycle_state` (`controller/support.rs`, task 021) already
names exactly the states where `submit` answers `Busy`. It is the classification
this slice queues behind; keep the two in agreement.

**Deadlines.** `MOUNT_DEADLINE_MS` 5 s, `SNAPSHOT_DEADLINE_MS` 2 s,
`RESUME_DEADLINE_MS` 1 s, `REFRESH_DEADLINE_MS` 2 s, `DESTROY_DEADLINE_MS` 1 s.
**`BarrierHeld` has none** — `next_deadline`
(`lifecycle/controller_support.rs:45-60`) returns `None` for it — which is why
RFC-047 §5.3 gives the queue its own.

## 3. One queue, not two · **[Binding]**

`waiting_command` is this design at depth 1, for one state, with a silent drop at
the end. **Replace it with the RFC-047 queue.** Do not add a second structure
beside it.

After this slice there is exactly one place where a user command waits, with one
set of rules for entering, draining, expiring and discarding it. Two would be the
"second mechanism to reason about" RFC-047 §10 Q3 rejects.

The existing behaviour that must survive the merge:

- a command submitted while the editor is `Mounting`/`Initializing` for the same
  document still runs when the editor becomes ready;
- `SubmitOutcome::WaitingForReady` still means "accepted, not yet run". Whether
  the outcome enum grows a variant for the new cases is **[Advisory]**; if it
  does, every existing `match` on it must be revisited, not given a wildcard.

## 4. The rules · **[Binding]**

1. **Accept instead of dropping.** Where `submit_with_focus` would answer `Busy`,
   enqueue and report acceptance.
2. **Depth 2, FIFO.** Order is preserved for entries that are not coalesced.
3. **Coalescing.** A newer `SwitchMode` replaces a queued `SwitchMode`. A newer
   `SaveNow` absorbs a queued `SaveNow`. Nothing else coalesces.
4. **Overflow rejects the newest**, with a discard reason (§5). Never evict the
   oldest.
5. **Expiry at 6 s** per entry, from when it was accepted. Derive it from
   `MOUNT_DEADLINE_MS` rather than writing `6_000` twice, and pin the
   relationship in a test (RFC-047 §10 Q4).
6. **Document-scoped entries are validated before they run.** `SaveNow`,
   `SaveAs`, `MoveSectionUp`, `MoveSectionDown` and `RestoreHistory` record the
   `SessionFingerprint`'s `document_id` at acceptance; if it no longer matches
   when the entry drains, discard it with a reason. `SwitchMode` and
   `OpenDocument` are not document-scoped.
   **This is the safety rule of the slice.** Saving document B because `Ctrl+S`
   was pressed while A was open would be data loss created by this feature.
7. **Never queue `Unavailable { retired: Some(_) }`.** Waiting cannot resolve it;
   it keeps today's immediate error.
8. **`NoOp` is unchanged** and stays silent.
9. **The focus token travels with the entry.** If the guard was diverted while it
   waited, the claim is refused and the command still runs. Do not re-arm a guard.

## 5. Discards must leave the controller, not vanish · **[Binding]**

Every path that removes an entry without running it must produce an outward,
typed record naming **the command and the reason**: overflow, expiry, document
changed, editor unavailable, relay lost, shutdown.

- Shape is **[Advisory]** — a new `ControllerAction` variant, or a value returned
  from the existing calls. It must be typed, not a string.
- **In this slice the host may only trace it.** No toast, no i18n: that is slice 2,
  which replaces the trace with the real message.
- **The five clearing sites in §2 must go through it.** Today they assign `None`.
  A queue that is emptied silently by a timeout is the same defect in a new place.

## 6. Draining · **[Binding] on the property**

An entry runs as soon as the controller would accept it, without waiting for the
next user action or an arbitrary delay.

- Attempt a drain after every handled event, on `tick`, and wherever
  `start_waiting_command` is called today.
- A drained entry goes through the **normal** submit path, so a protected command
  is still protected and the barrier is unchanged (RFC-047 §4).
- Draining must not recurse into itself or re-enqueue an entry it just took.
- **No timer drives the queue.** `tick` already exists; this adds no new clock.

## 7. Non-change scope · **[Binding]**

- The lifecycle reducer (`source_sync/lifecycle*`): states, deadlines, the
  barrier, `begin_mount`'s `waiting` mount intent — which is a **mount** intent,
  not a user command, and is not what this slice replaces.
- Toasts, i18n, and any component. Slice 2 owns the message.
- The RFC-044 harness, including task 021's settle gate and the phases.
- `bridge::trace` call sites other than the one this replaces.
- Version numbers, `CHANGELOG.md`, `docs/`.

## 8. Required tests · **[Binding]**

All headless, in the existing controller test modules.

1. **Acceptance** in every state `busy_lifecycle_state` reports, including
   `BarrierHeld`, and non-acceptance for `Unavailable { retired: Some(_) }`.
2. **FIFO order**, for two commands that do not coalesce.
3. **Coalescing**: `SwitchMode` replaced by a newer one, so it runs once with the
   newer mode; the same for `SaveNow`.
4. **Overflow** rejects the newest, and the record names it.
5. **Expiry** at the derived deadline, with the record. Plus the test pinning the
   deadline to `MOUNT_DEADLINE_MS`.
6. **§4.6's safety rule**: a queued `SaveNow` for document A, with the open
   document changed to B, is discarded and recorded — **and does not save B**.
   Assert what did *not* happen, not only the record.
7. **Each of §5's clearing sites** produces a record rather than a silent `None`.
8. **The preserved behaviour of §3**: a command submitted during `Mounting` for
   the same document still runs on `EditorReady`.
9. **The drain is prompt**: after the event that settles the controller, the entry
   has been submitted without another user action.

Prove every one able to fail by mutation, one at a time, restoring between them
and comparing byte for byte. §8.6 and §8.4 matter most: a queue that silently
keeps its promise only in the happy path is what this RFC exists to prevent.

## 9. Acceptance criteria · **[Binding]**

1. `waiting_command` is gone as a separate mechanism; one queue remains.
2. §4's rules hold, each with a test from §8.
3. Every removal without execution yields a typed record; no site assigns the
   queue away silently.
4. No UI, i18n or harness change.
5. `cargo test --workspace`, fmt, clippy `-D warnings` clean; `Cargo.lock`
   unchanged; no file over the 500 ELOC gate — check it **locally** before
   pushing (task 021's first push failed on exactly that).
6. CI green, including the RFC-044 shell-behaviour outcome line, which must still
   read `success`.

## 10. Evidence · **[Binding]**

- The mutation table: each test, the mutation, and the named failure.
- The §8.6 result quoted in full, including the assertion that B was not written.
- The list of call sites that changed from a silent clear to a recorded discard.
- Before/after test counts, and the ELOC figures for every file you touched.
- Confirmation that the RFC-044 outcome line is unchanged.

## 11. CI, merge, review request

Branch, commit, push and draft PR are pre-authorized. Merging needs an explicit
instruction under the merge procedure.

Write the review request to
`.git-exclude/review-request/<date>-rfc-047-slice-1-the-queue.md`. **Lead with
§8.6**, the queued save that must not write the wrong document, and with the list
of silent clears that now report.
