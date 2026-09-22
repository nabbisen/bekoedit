# RFC-047 handoff — slice 2: the report

**Governing RFC:** [RFC-047](../../done/RFC-047-user-commands-during-editor-transitions.md) §5.5 (as amended 2026-09-22), §7, §9
**Slice:** 2 of 2 — what the user sees, and the WebView proof.
**Baseline:** `main` at `7a0ca07` or later, with slice 1 and task 022 merged. If
`main` moves in a file this slice touches, merge `origin/main` in; never rebase.
**Status:** inherited from RFC-047 (Implemented, on `main`; accepted 2026-09-22)
**Date:** 2026-09-22

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide.
An advisory mechanism you may replace with your own reasoning, without asking.

Integration follows `.git-exclude/governance/merge-procedure.md`.

## 1. Purpose

Slice 1 made the controller keep every command and record every discard. Today
those records are only traced (`host.rs`'s `trace_discards`), which a release
build writes nowhere a user can see. This slice tells the user, closes the one
remaining silent drop, and proves the queue in a real WebView.

## 2. What exists · verified on `7a0ca07`

- **The outbox.** `SourceSyncState::drain_discards() -> Vec<QueueDiscard>` and
  `has_discards()`. `QueueDiscard { command, reason, focus_token }`, with
  `DiscardReason::{Overflow, Expired, DocumentChanged, EditorUnavailable,
  RelayLost, Shutdown}`.
- **Where they are read.** `host.rs`'s `trace_discards`, called from the dispatch
  effect and after `shutdown`.
- **Toasts are already announced.** `ToastLayer` renders `role="status"` with
  `aria-live="polite"`, so a `Warning` toast reaches a screen reader without new
  markup. `push_toast(&mut toasts, ToastKind::Warning, message)`.
- **i18n.** `tr(lang, key)`, both arms required; `all_keys()` in `tests.rs` derives
  the key list from `tr_en`'s source, so new keys are picked up automatically by
  the parity and plain-language guards.
- **Mode names already exist** as keys: `mode.text`, `mode.preview`, `mode.form`,
  and Split's.

## 3. The message · **[Binding]**

**One sentence: what did not happen, then why.** "Could not switch to Form — the
editor was busy." Never a lifecycle state, a command name, an enum variant or a
reason code.

1. **The action half comes from the command**, and must name the thing the user
   acted on:
   - `SwitchMode(m)` → the mode's own translated name;
   - `OpenDocument(path)` → the **file name**, not the path;
   - `SaveNow` / `SaveAs` → saving, and for `SaveAs` the target file name;
   - `NewUntitled`, `OpenWorkspace`, `CloseWorkspace`, `OpenSettings`,
     `MoveSectionUp`/`Down`, `RestoreHistory` → their own phrasing.

   Cover **every** `SourceCommand` variant, with no wildcard arm, so a new command
   cannot reach a user as "could not do something".
2. **The reason half comes from `DiscardReason`**, in the user's terms:
   - `Overflow`, `Expired` → the editor was busy;
   - `DocumentChanged` → the document changed before it could run — this one must
     be unmistakable, because it is the case where **not** acting protected the
     user's file (RFC-047 §5.4);
   - `EditorUnavailable`, `RelayLost` → the editor stopped responding.
3. **`Shutdown` is silent** (§5.5 as amended). The window is closing.
4. **Both languages**, through the existing guards. Plain language: if a term the
   guard rejects is genuinely needed, take the per-key exception route with a
   comment, as task 020 did for "WebView" — do not reword into vagueness.

## 4. Grouping · **[Binding]**

Several commands can be discarded by one event. **Group by reason and report once
per reason**, naming how many when it is more than one. A queue of two plus
whatever the host had not yet run must not produce a stack of toasts.

Singular and plural must both read naturally in EN and JA. Japanese has no plural
agreement, so do not build the sentence by string concatenation that assumes
English grammar.

## 5. The remaining silent drop · **[Binding]**

`relay_disconnected` (`controller/support.rs`) calls `self.actions.clear()`. An
`Execute` the host has not run yet is a command the user asked for, already past
the queue, and it disappears with no record. Slice 1's review recorded it; it is
this slice's to close.

- Record those commands as discards, with `RelayLost`, through the same outbox.
- Only user commands: a `Lifecycle` action is the controller's own business and
  must not be reported.
- `shutdown` also clears actions. Those stay silent, per §3.3.

## 6. Wiring · **[Binding] on the property**

- `trace_discards` becomes the reporting path. Keep the trace line as well: it
  carries the reason code for a developer, while the toast carries the sentence.
- **Drain the outbox exactly once per event.** A discard reported twice is worse
  than one reported late.
- The controller does not cancel focus tokens for discards (slice 1, §4.6), and
  this slice does not change that.

## 7. The WebView phase · **[Binding]**

RFC-047 §7's phase, in RFC-044's second run: **click Preview and then Text in the
same exchange**, and assert Text wins and the editor takes focus.

- One exchange, deliberately: that is the shape that raced before task 021, so the
  settle gate cannot mask it. This is the executable proof that the queue works in
  a real WebView, and that the harness gate is not hiding the defect.
- Task 022 means the editor must also **take focus**, so assert that too.
- Place it after the existing tab phases. Extend `ShellBehaviourPhase`, `next()`,
  `expected_milestone`, `EXPECTED_MILESTONES`, `TERMINAL_STAGE` if it becomes last,
  and the driver's `phases` list together.

**This phase lands on a step that now blocks** (`main` `47e6a04`). Under RFC-044
§10 as amended 2026-09-22, that means it merges blocking, carrying its own
mutation evidence. If you believe it needs a soak behind `continue-on-error`
instead, **say so in the review request with your reasoning** rather than adding
the flag.

**Its mutation** is the one that matters: restore the old behaviour for the case
under test — a `SwitchMode` arriving while the controller is `Unmounting` is
dropped rather than queued — and show the phase failing, naming itself. Without
that, the phase only proves the app works today.

## 8. Non-change scope · **[Binding]**

- The queue's semantics: depth, coalescing, expiry, the document-scoped safety
  rule, and task 022's effective-target rule.
- The lifecycle reducer.
- `ToastLayer`'s markup, its four-second dismissal, and the toast vocabulary. Use
  `Warning`; do not add a kind.
- Focus-token handling for discards.
- Version numbers and `CHANGELOG.md` — task 013 carries the entry, and I am adding
  it.
- RFC-044's existing phases.

## 9. Required tests · **[Binding]**

1. **The message, headless**: every `SourceCommand` variant produces its action
   phrase; every reported `DiscardReason` produces its clause; both languages;
   `Shutdown` produces nothing.
2. **Grouping**: two discards with one reason report once, naming two; two
   reasons report twice; one reports once with singular phrasing.
3. **§5**: losing the relay with an unexecuted `Execute` records it as `RelayLost`,
   and a `Lifecycle` action does not.
4. **Drained once**: a second pass over the same event reports nothing.
5. **The i18n guards** pass, including plain language.
6. **The WebView phase** green in CI, plus §7's mutation failing and naming it.
7. Driver unit tests for the new phase, in the existing shared-fake suite.

Prove each able to fail by mutation, one at a time, restoring between them and
comparing byte for byte. Run `clippy --all-targets -D warnings` and the ELOC check
**locally, in the same chain as the tests**, before pushing.

## 10. Acceptance criteria · **[Binding]**

1. Every reported discard reaches the user as one plain sentence naming the action
   and the reason; `Shutdown` reports nothing.
2. No `SourceCommand` or reported `DiscardReason` falls through a wildcard.
3. Grouping per §4, reading correctly in both languages.
4. §5's silent drop is closed.
5. The WebView phase is in the second run and green, with its mutation recorded.
6. `cargo test --workspace`, fmt, clippy, the i18n guards and the ELOC gate all
   clean; `Cargo.lock` unchanged.

## 11. Evidence · **[Binding]**

- The exact EN and JA sentences for **each** command and each reported reason, in
  a table. I will read them as a user would, so write them as they will appear.
- The mutation table.
- The WebView phase's green run and its mutation run, both quoted from the log,
  with the outcome line.
- Whether you took the plain-language exception route, and for which term.
- Before/after test counts and ELOC for every file touched.

## 12. CI, merge, review request

Branch, commit, push and draft PR are pre-authorized. Merging needs an explicit
instruction under the merge procedure.

Write the review request to
`.git-exclude/review-request/<date>-rfc-047-slice-2-the-report.md`. **Lead with
§11's sentence table** — this slice is judged on what a user reads — and with the
WebView phase's mutation failing.
