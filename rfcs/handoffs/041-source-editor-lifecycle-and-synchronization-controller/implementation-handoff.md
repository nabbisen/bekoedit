# RFC-041 implementation handoff

## 1. Summary

Implement the reviewed form of
[RFC-041](../../proposed/RFC-041-source-editor-lifecycle-and-synchronization-controller.md):
replace optimistic CodeMirror registration and distributed Dioxus effects with
one explicit Rust-owned source-editor lifecycle controller.

This handoff is design-stage guidance, not implementation authorization. Its
status is inherited from the Proposed RFC.

## 2. Scope followed

The handoff covers:

- app-level editor bundle readiness;
- application-root controller lifetime across all child-screen replacement;
- per-mount Text/Split instance identity;
- relay-before-init and validated Ready handshake;
- existing source-sync barrier integration;
- explicit snapshot terminal results;
- explicit BarrierHeld and deadline-bounded ResumePending around protected
  command execution;
- total post-snapshot command success/failure disposition table;
- acknowledged canonical refresh/rebase with epoch rollover;
- serialized singleton destroy/init and one-use takeover;
- bridge protocol version 2 and first-terminal-wins deadlines;
- explicit unmount/destroy/abort;
- Settings and other full-screen unmount paths;
- shared Text/Split hosting;
- module splitting required by the 500-ELOC gate;
- pure, JavaScript, and desktop regression evidence.

Out of scope:

- replacing CodeMirror;
- changing Markdown parsing;
- unrelated architect-review blockers;
- releases, commits, tags, or pushes.

## 3. Files changed

Design-stage files created or updated:

- `rfcs/proposed/RFC-041-source-editor-lifecycle-and-synchronization-controller.md`
- `rfcs/handoffs/041-source-editor-lifecycle-and-synchronization-controller/implementation-handoff.md`
- `rfcs/README.md`
- `.git-exclude/review-request/2026-07-14-rfc-041-source-editor-lifecycle-design.md`
- `.git-exclude/review-request/2026-07-14-rfc-041-source-editor-lifecycle-design-rereview.md`
- `.git-exclude/review-request/2026-07-14-rfc-041-source-editor-lifecycle-design-final-rereview.md`

Expected implementation areas after approval:

- `crates/bekoedit-app/src/source_sync.rs` and new `source_sync/` modules;
- `crates/bekoedit-ui-contract/src/source_editor.rs` and bridge version tests;
- `crates/bekoedit-app/src/components/text_mode.rs`;
- `crates/bekoedit-app/src/components/split_mode.rs`;
- a shared source-editor host/controller component or hook;
- `crates/bekoedit-app/src/app.rs` and Settings navigation callers;
- `crates/bekoedit-app/src/bridge.rs`;
- `crates/bekoedit-app/js/src/editor.js`;
- `crates/bekoedit-app/assets/editor-bundle.js` after the JS build;
- app, protocol, and lifecycle tests.
- CI/release app-test gates and a Linux real-WebView regression harness.

The source-sync barrier is committed in `776dd26`; the eight tracing/diagnostic
application changes and initial RFC package are committed in `67ea8dc`. The
revision-2 package is committed in `e92cbfe`. Revision 3 changes only RFC/handoff
documents. Future implementation must still decide, file by file, which
diagnostic behavior to retain or replace.

## 4. Design decisions and assumptions

- CodeMirror is not identified as the defect; readiness at the
  Dioxus/JavaScript boundary is.
- Keep one JavaScript adapter/view, but assign one Rust controller and a fresh
  identity to each mount.
- Host that controller and its relay at app root so they survive MainShell,
  mode, Settings, document, and workspace replacement.
- Bundle readiness is application-level and persistent.
- Editor readiness is instance-level and becomes true only after a validated
  JavaScript Ready message.
- Relay readiness and bundle readiness are prerequisites for init; their
  relative completion order may vary.
- `ActiveSourceEditor` means Ready and snapshot-capable, not merely intended to
  mount.
- `EditorInstanceId` identifies a physical mount; `SourceEpoch` identifies a
  canonical stream/rebase generation within that mount.
- A protected command may wait for the current mount, but remains single-flight
  and bounded.
- Bridge schema version 2 is an incompatible, exact-match protocol whose typed
  source-editor family lives in `bekoedit-ui-contract`.
- Every operation uses first-terminal-wins reducer semantics with separate
  mount, snapshot, resume, refresh, and destroy deadlines.
- A successful protected snapshot holds editor input until Resume, canonical
  ApplyDocument acknowledgement, or Destroy.
- Ready never describes an instance whose hold state is unknown; failed/lost
  snapshot and resume paths terminalize through ResumePending or Unavailable.
- Every protected command maps success, unchanged-session failure, and
  ambiguous/partial change to Resume, Refresh, Destroy, or Unavailable.
- History/Outline canonical mutation invalidates the old epoch and becomes
  Ready only after correlated DocumentApplied.
- Singleton replacement waits for matched Destroyed or uses one correlated,
  one-use takeover permit after destroy timeout.
- Settings is protected because it unmounts `MainShell`.
- Startup recovery remains a structural no-active-editor exception.
- Rust lifecycle transitions must be pure-testable; WebView behavior still
  requires desktop evidence.

Rereview confirmations are listed in RFC-041 section 16. Do not silently choose
different semantics during implementation; amend the RFC first.

## 5. Tests and gates run

No Rust build, test, lint, JavaScript build, or desktop manual test was run for
this design-only change. No runtime source was changed, so implementation gates
would not provide evidence for the proposed protocol.

Observed document validation on 2026-07-14:

```text
bash scripts/check-rfcs.sh — passed, 0 errors
git diff --check — passed for tracked changes
explicit whitespace checks — passed for all newly generated artifacts
```

Implementation gates after approval should include at minimum:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/check-rfcs.sh
mdbook build docs
git diff --check
cargo build -p bekoedit
./target/debug/bekoedit --headless-smoke
```

Also run the locked JavaScript install/build workflow used by the repository,
the exact CI ELOC check, and the RFC-041 desktop regression checklist. Do not
claim lifecycle acceptance from headless tests alone.

## 6. Generated artifacts

- Proposed RFC-041.
- This implementation handoff.
- Architecture review request for the design package.
- Focused architecture rereview request for revision 2.
- Final focused architecture rereview request for revision 3.

No binary, package, release archive, commit, tag, or push was generated.

## 7. Known limitations

- The attempted live trace exited before the owner interaction sequence, so no
  complete event log for Issue 4 was captured.
- Static code establishes the initial bundle/init false-readiness failure and
  the barrier timeout path. It does not distinguish every possible
  relay/identity timing branch after Settings remount.
- The exact Dioxus evaluator shape for the app-root controller remains an
  implementation detail, provided it satisfies RFC-041 and its host-lifetime
  test.
- Cross-platform WebView timing cannot be proven by Rust unit tests.
- The repository currently has other architect-review blockers outside this
  RFC's scope.

## 8. Recommended next step

Obtain final architecture rereview of RFC-041 revision 3 and this handoff. After
an acceptable verdict, implement in the RFC rollout order, beginning with
bridge version types and the pure lifecycle reducer rather than editing the
current Dioxus effects in place.
