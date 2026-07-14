# RFC-041: Source Editor Lifecycle and Synchronization Controller

**Project:** bekoedit
**Status:** Proposed
**Track:** Source safety / WebView lifecycle
**Priority:** Critical
**Date:** 2026-07-14
**Revision:** 2 — resolves the 2026-07-14 initial architecture review
**Related RFCs:** [RFC-002](../done/RFC-002-runtime-architecture-and-webview-boundary.md), [RFC-011](../done/RFC-011-text-mode-with-codemirror-6.md), [RFC-019](../done/RFC-019-mode-switching-and-projection-synchronization.md)
**Implementation handoff:** [RFC-041 implementation handoff](../handoffs/041-source-editor-lifecycle-and-synchronization-controller/implementation-handoff.md)

---

## 1. Summary

Define one explicit lifecycle controller for the CodeMirror-backed Text and
Split editors. Rust must not treat a source editor as active or
snapshot-capable until the JavaScript bundle, relay, CodeMirror view, document
identity, and editor instance identity have completed a validated handshake.

This RFC extends the accepted source-sync barrier design. It addresses the
startup and remount failure where Rust registers an editor before
`window.__bk.init(...)` can run, then waits for snapshots from a view that does
not exist.

Revision 2 fixes the controller host above all controlled screen replacement,
adds acknowledged canonical refresh/rebase, serializes singleton replacement,
and defines the incompatible bridge protocol version 2 with operation-specific
deadlines and first-terminal-wins semantics.

## 2. Incident and root cause

Owner reproduction on 2026-07-14:

1. Start the application and click **New**.
2. Text Mode is visible but is not editable.
3. Repeated protected actions produce busy and timeout messages.
4. Open Settings and return.
5. Text Mode becomes editable after the remount.
6. Typing appears to work, but Preview does not complete reliably.

The current component lifecycle has no readiness invariant:

- `document::Script` starts loading the CodeMirror bundle at runtime.
- a Dioxus effect registers an active source editor immediately;
- the effect executes `if (window.__bk) { window.__bk.init(...) }`;
- absence of `window.__bk` is a silent no-op;
- relay installation and editor initialization are independent;
- JavaScript relay sends use optional chaining and may silently drop messages;
- the JavaScript `ready` message does not change Rust lifecycle state;
- snapshot requests are marked sent even if no JavaScript API handled them;
- unmounting Text/Split has no matched destroy and pending-command abort.

Settings changes the timing by unmounting Text Mode while the bundle finishes
loading. The remount can therefore succeed accidentally. This is an
application-level Dioxus/JavaScript lifecycle bug, not evidence of a CodeMirror
editing defect.

## 3. Goals

- Make the first New -> Text mount predictably editable.
- Give Rust an observable, validated definition of editor readiness.
- Sequence relay installation and CodeMirror initialization.
- Preserve the accepted source-sync barrier for every protected command.
- Make Text and Split use the same lifecycle and protocol.
- Destroy or invalidate every mounted editor instance explicitly.
- Prevent stale instances, epochs, requests, timers, and relay messages from
  affecting a newer editor.
- Make every failure terminal, visible, and retryable without a busy loop.
- Keep canonical Markdown and filesystem authority in Rust.

## 4. Non-goals

- Replacing CodeMirror 6.
- Introducing a general-purpose WebView RPC framework.
- Allowing more than one simultaneously editable CodeMirror instance.
- Redesigning Markdown parsing or source-preserving mutations.
- Treating a timeout as successful synchronization.
- Starting implementation before this RFC receives architecture approval.

## 5. Architectural invariants

### 5.1 Canonical-source invariant

Rust canonical text remains the durable source of truth. CodeMirror may hold a
newer working copy only while its current editor instance is mounted.

### 5.2 Readiness invariant

`Ready` means all of the following are proven for one identity tuple:

```text
(editor_instance_id, editor_id, document_id, epoch)
```

- the bridge bundle API version is supported;
- the instance relay is installed and can reach Rust;
- the CodeMirror container exists;
- one CodeMirror `EditorView` was constructed for the instance;
- JavaScript sent `EditorReady` through the instance relay;
- Rust validated every identity field.

Mounted, registered, rendered, bundle-requested, and `eval`-queued do not mean
Ready.

### 5.3 Single-owner invariant

One Rust lifecycle controller owns the single CodeMirror JavaScript adapter.
TextMode and SplitMode provide mount intent and rendering surfaces; they do not
independently infer bridge readiness through unrelated effects.

### 5.4 Protected-command invariant

A command that consumes, mutates, saves, replaces, or unmounts source state may
execute only after:

- no source editor working copy exists; or
- the current Ready editor supplied a correlated snapshot that Rust accepted.

Failure keeps the source command unexecuted.

### 5.5 Terminal-request invariant

Every accepted snapshot request produces exactly one terminal result for the
matching request and editor instance:

- `Snapshot`;
- `SnapshotBlocked`; or
- a controller-generated timeout/unavailable result.

A pending request must never survive loss of its owning editor instance.

### 5.6 Stale-message invariant

Messages from an old instance, document, editor kind, epoch, or request cannot
complete or mutate the current lifecycle.

## 6. Ownership model

### 6.1 App-level bundle state

Load the editor bundle once at application-root scope rather than from each
Text/Split mount. Loading may still be asynchronous. The controller therefore
tracks an app-level state:

```rust
enum EditorBundleState {
    Loading,
    Ready { protocol_version: u32 },
    Failed { reason: BridgeFailure },
}
```

Bundle readiness must be persistent and queryable. It must not depend on a
one-time event that can be lost before Rust installs a relay. A bounded probe or
equivalent persistent boot handshake may establish `Ready`.

### 6.2 Instance-level lifecycle

Each Text or Split mount receives a new `editor_instance_id`. An epoch remains
the source-stream generation within the Rust synchronization model; the
instance ID identifies the physical CodeMirror ownership lifetime.

```rust
enum SourceEditorLifecycle {
    Unmounted,
    Mounting(MountingEditor),
    Initializing(MountingEditor),
    Ready(ReadyEditor),
    SnapshotPending {
        editor: ReadyEditor,
        request: SnapshotRequest,
    },
    RefreshPending {
        editor: RefreshingEditor,
        request: RefreshRequest,
    },
    Unmounting(EditorIdentity),
    Unavailable {
        identity: Option<EditorIdentity>,
        reason: BridgeFailure,
    },
}
```

`ActiveSourceEditor` must mean Ready and snapshot-capable. If compatibility
requires retaining the name, its construction must be private to the validated
`EditorReady` transition.

`EditorInstanceId` and `SourceEpoch` are distinct opaque monotonic newtypes.
The controller allocates both without process-lifetime reuse. An instance ID
identifies one physical view ownership lifetime. An epoch identifies one
accepted canonical document stream within that instance and changes on a
canonical refresh/rebase.

### 6.3 Controller host lifetime

The sole controller, lifecycle reducer state, relay receiver, operation
deadlines, and identity allocators are hosted at application-root scope. That
scope must outlive:

- `MainShell`;
- Text, Split, Preview, and Form components;
- Settings replacement and return;
- document/session replacement;
- workspace replacement.

The root controller is created once for the running application. Child screens
receive a controller handle through context and may submit typed intent only.
They may not own relay coroutines, lifecycle state, request-dispatch effects,
timeouts, or identity allocation.

Editor-instance unmount does not shut down the controller. Application exit is
the separate controller-shutdown boundary: reject all pending operations,
invalidate the current instance, make one best-effort matched destroy request,
close the relay/evaluator, and stop timers. Application shutdown must never
execute a pending protected command.

### 6.4 Component responsibilities

TextMode and SplitMode:

- render a stable container;
- announce mount intent with editor kind, document identity, revision, text,
  and container ID;
- expose component drop to the controller;
- display loading/unavailable state without accepting input prematurely.

The controller:

- allocates identities;
- installs and owns the relay;
- waits for supported bundle readiness;
- initializes and destroys JavaScript instances;
- validates all inbound messages;
- drives snapshot requests and protected-command completion;
- owns lifecycle timeouts and failure recovery.

Only the pure lifecycle reducer may mutate lifecycle or barrier state. The
controller converts Dioxus, eval, relay, and timer inputs into typed reducer
events, then executes the effects emitted by the reducer.

JavaScript:

- owns only the live CodeMirror view and UI-local editing mechanics;
- never chooses canonical revision or persistence actions;
- returns explicit protocol results instead of silent optional calls.

## 7. Lifecycle transitions

### 7.1 Mount and initialize

1. A source component renders its container and submits mount intent.
2. Rust allocates a fresh instance ID and epoch and enters `Mounting`.
3. The controller installs an instance relay.
4. The relay sends a persistent/observable `RelayReady` acknowledgement.
5. The controller waits for both `RelayReady` and supported bundle readiness.
6. Only then may the controller call JavaScript `init`.
7. JavaScript validates that no different instance owns the singleton,
   validates the container, constructs exactly one view, then emits
   `EditorReady`.
8. Rust validates the identity and transitions to `Ready`.
9. The controller may focus the new editor after Ready.

Initialization failure transitions to `Unavailable`, clears any queued
protected command with visible failure, and leaves a retry path. It must not
create an active editor record.

An exact duplicate init for an already-live instance is idempotent: JavaScript
does not reconstruct the view and re-emits the matching `EditorReady` result
with `reused = true`. An init using the same instance ID with different
identity fields is rejected. An init for a different instance while an old
instance remains active is rejected with `InstanceAlreadyActive` unless it
carries the one-use takeover permit defined in section 7.6.

### 7.2 Protected command while mounting

At most one protected command may wait for the current mount to become Ready.
The command does not create a snapshot request until Ready is validated.

- Ready before the mount deadline: request a snapshot and continue normally.
- Mount failure or timeout: reject the command, clear single-flight state, and
  show an editor-unavailable message.
- A second protected command: report busy once without replacing the first.

This rule makes an immediate Preview click safe even if CodeMirror is still
starting.

### 7.3 Ordinary change

An ordinary CodeMirror change includes the complete editor identity, sequence,
text, and composition state. Rust accepts it only under the existing
document/epoch/sequence/revision/conflict checks.

The 100 ms debounce may remain as a performance detail. It cannot be the
correctness mechanism for protected commands.

### 7.4 Snapshot request

The controller creates one request ID and dispatches it once to the current
Ready instance. JavaScript must not use optional chaining or a guarded no-op.
It returns one correlated result even when text is unchanged.

Suggested messages:

```json
{
  "type": "requestSnapshot",
  "protocolVersion": 2,
  "requestId": 99,
  "instanceId": 501,
  "editorId": "text",
  "docId": 123,
  "epoch": 7
}
```

```json
{
  "type": "snapshot",
  "protocolVersion": 2,
  "requestId": 99,
  "instanceId": 501,
  "editorId": "text",
  "docId": 123,
  "epoch": 7,
  "seq": 18,
  "text": "...",
  "composing": false
}
```

Blocked reasons include `compositionActive`, `editorUnavailable`,
`identityMismatch`, and `bridgeError`.

Snapshot acceptance or no-op acceptance completes the protected command.
Blocked, rejected, unavailable, or timeout results clear the pending command
without executing it.

After JavaScript captures a snapshot for a protected command, it places that
instance in a short barrier hold: user document transactions are disabled until
the controller sends one of `ResumeEditing`, `ApplyDocument`, or `Destroy`.
This prevents input entered after the accepted snapshot from racing a mode
switch or canonical Rust mutation. A blocked snapshot does not enter the hold.

### 7.5 Canonical refresh and rebase

Some protected commands mutate canonical Rust text while Text/Split remains
mounted, including History restore and Outline section moves. After the command
mutation succeeds, the old editor stream must not become Ready again with stale
text.

The transition is:

```text
Ready(old epoch)
  -> SnapshotPending(barrier hold)
  -> RefreshPending(old epoch invalid, new epoch allocated)
  -> Ready(new epoch, acknowledged canonical revision)
```

Rules:

1. Snapshot acceptance completes synchronization but retains the barrier hold.
2. Rust executes the protected canonical mutation.
3. Before dispatching refresh, the controller invalidates snapshot capability
   for the old epoch and allocates a fresh `SourceEpoch` for the same instance.
4. The controller sends `ApplyDocument` with operation ID, instance ID, editor
   ID, document ID, old and new epoch, canonical revision, and canonical text.
5. JavaScript verifies the current instance and old epoch, cancels pending
   debounce/composition state, installs the canonical document without
   publishing a user change, and replies `DocumentApplied` with the correlated
   operation ID, new epoch, and canonical revision.
6. Only a matching acknowledgement transitions to Ready and re-enables input.

If canonical text is unchanged, the same acknowledged transition still rolls
the epoch when a protected Rust mutation requested a refresh. This keeps one
unambiguous rule; no local no-op shortcut may reactivate the old stream.

Refresh failure or timeout transitions to `Unavailable`, retains Rust canonical
text, clears the protected operation, and never accepts old-epoch changes.
Retry performs a fresh instance mount from canonical text. A protected command
submitted during `RefreshPending` remains single-flight busy and cannot extend
the refresh deadline.

Late old-epoch debounce, stale refresh acknowledgement, wrong revision, and
wrong operation ID are traced stale no-ops. Active IME composition should have
blocked the preceding snapshot; if composition is nevertheless reported at
refresh dispatch, refresh fails closed and remounts from canonical text rather
than publishing or preserving partial composition text.

Save operations that do not change canonical text release the barrier hold
with `ResumeEditing` only after save completion, provided instance, epoch, and
revision remain current. Mode, Settings, document, and workspace transitions
proceed to matched destroy instead of resuming.

### 7.6 Unmount, destroy, and serialized replacement

Component drop submits `Unmount(instance_id)`.

1. Rust invalidates readiness for that instance immediately.
2. A pending snapshot/protected command owned by the instance is rejected and
   cleared unless unmount follows successful command completion.
3. Rust invokes `destroy(instance_id)`.
4. JavaScript clears the debounce timer, composition state, view, current
   identity, and relay reference when the identity matches.
5. A matched `Destroyed` acknowledgement completes `Unmounted`.
6. A destroy timeout still leaves the Rust identity invalid; late messages are
   rejected.

`destroy` is idempotent for an already-destroyed matching instance. It must not
destroy a newer instance.

New ownership is serialized. A new mount normally waits for matched
`Destroyed`. If the destroy deadline expires, Rust still keeps the old identity
invalid and may issue a one-use `TakeoverPermit` containing the retired
instance ID, replacement instance ID, and controller-allocated nonce. JavaScript
accepts it only when its current singleton still matches the retired instance;
it clears that view and consumes the permit before initializing the named
replacement. A permit cannot authorize any other replacement.

A late drop/destroy for the retired instance is a stale no-op after replacement.
A new mount received during `Unmounting` is recorded as the sole waiting mount;
it cannot initialize until destroy succeeds or the controller issues the
takeover permit. Additional mounts replace neither owner nor waiting mount and
fail visibly.

## 8. Settings and other screen transitions

Opening Settings replaces `MainShell` and unmounts Text/Split. It is therefore
a protected command while a source editor working copy exists.

Add a navigation command such as:

```rust
SourceCommand::OpenSettings
```

It synchronizes the Ready editor before changing `settings_open`. Closing
Settings creates a normal fresh mount; it must not rely on a previous global
view. Start-screen actions remain immediate because no source editor is
mounted.

Any future full-screen navigation that unmounts a source editor must use the
same rule.

## 9. Failure and user experience

- While mounting, show a quiet loading state in the editor surface.
- Do not show a writable ARIA textbox until the CodeMirror view is Ready.
- On initialization failure, show one actionable error and a Retry action.
- A timeout must clear the pending operation immediately.
- Repeated clicks must not stack duplicate timeout toasts.
- Preview, Save, navigation, and source mutations must never proceed using
  stale canonical text after lifecycle failure.
- Successful Ready should focus the editor when the mount originated from New
  or a source-mode switch, unless the user moved focus elsewhere meanwhile.
- Focus requests carry a controller-allocated interaction token. Ready may
  focus only when that token is still current and focus remains on the editor
  launch surface; a late Ready must not steal focus from Settings, a recovery
  action, a dialog, or another control.
- Durable loading/failure/Retry UI is one lifecycle status surface keyed by
  instance and operation. A toast may announce a transition once, but repeated
  clicks must not create the durable state or duplicate announcements.

## 10. Protocol rules

### 10.1 Version and location

RFC-041 is an incompatible bridge change and bumps the project bridge schema
from 1 to 2.

- Rust authority: `bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION = 2`.
- JavaScript authority: one exported `BRIDGE_SCHEMA_VERSION = 2` constant in
  `js/src/editor.js`, included in the built bundle.
- A contract test reads/queries both authorities and fails if they differ.

The typed source-editor wire family lives in
`crates/bekoedit-ui-contract/src/source_editor.rs`. TextMode and SplitMode use
that family through the controller; they must not define local duplicate
deserializers. JavaScript mirrors the same discriminants and fields, and its
adapter tests serve as the cross-language compatibility gate.

Every request and result carries `protocolVersion`, `operationId`, and the full
identity applicable to that operation. Rust and JavaScript reject every version
other than exactly 2 before lifecycle or document mutation. There is no
version-1 compatibility fallback because the bundle and Rust binary ship
together.

### 10.2 Typed operation family

The protocol contains these operations and correlated results:

| Operation | Required result |
|---|---|
| `ProbeBundle` | `BundleReady` or `BundleFailed` |
| `InstallRelay` | `RelayReady` or `RelayFailed` |
| `InitEditor` | `EditorReady` or `InitFailed` |
| `EditorChange` | inbound ordinary event; identity/sequence validated |
| `RequestSnapshot` | `Snapshot` or `SnapshotBlocked` |
| `ResumeEditing` | `EditingResumed` or `ResumeFailed` |
| `ApplyDocument` | `DocumentApplied` or `ApplyDocumentFailed` |
| `DestroyEditor` | `Destroyed` or `DestroyFailed` |
| `Trace` | non-authoritative, content-safe diagnostic event |

`InitFailed` includes at least unsupported version, missing container,
`InstanceAlreadyActive`, identity mismatch, and bridge error. Refresh and
destroy failures likewise carry operation and instance identity. A
`TakeoverPermit` is accepted only as part of `InitEditor` and only under section
7.6 rules.

Dispatch/eval queuing is not delivery proof and never constitutes an operation
result. Missing delivery ends through the matching deadline as unavailable or
timeout.

### 10.3 First-terminal-wins

“One terminal result” describes controller state commitment, not the physical
number of wire messages. For each operation ID, the reducer atomically accepts
the first matching terminal event and removes/terminalizes the pending
operation. Duplicate results, a JavaScript result arriving after timeout, and a
timer arriving after a JavaScript result are stale no-ops recorded only through
content-safe trace evidence. They cannot execute a command, change readiness,
or extend a deadline twice.

### 10.4 Deadlines

Use separate named deadline classes, initially:

```text
MOUNT_DEADLINE_MS = 5_000
SNAPSHOT_DEADLINE_MS = 2_000
REFRESH_DEADLINE_MS = 2_000
DESTROY_DEADLINE_MS = 1_000
```

- Mount timeout: enter `Unavailable`, reject the mount-bound protected command,
  and offer fresh-instance Retry.
- Snapshot timeout: return the current Ready instance from barrier hold if the
  adapter can acknowledge resume; otherwise enter `Unavailable`; never execute
  the protected command.
- Refresh timeout: enter `Unavailable`, retain Rust canonical text, reject old
  epoch messages, and require remount from canonical text.
- Destroy timeout: keep the retired identity invalid and either finish without
  replacement or use the single waiting mount's takeover permit.

A second user action never extends an existing deadline. Tests use injected
clock events; production orchestration owns timers at app-root controller
scope.

### 10.5 General rules

- JavaScript reports missing containers, missing views, relay loss, and
  identity conflicts explicitly whenever transport is available.
- Relay disappearance is a controller failure, not a silent dropped message.
- The controller is the only place allowed to translate bridge events into
  source-sync state transitions.
- Trace logging remains opt-in and content-safe: identities, revisions,
  sequence numbers, lengths, transitions, and reasons; never document text.

## 11. Protected command inventory

RFC-041 retains the accepted expanded barrier inventory and adds lifecycle
navigation:

- mode changes replacing Text/Split;
- Save and Save As;
- Open document from Explorer, Search, or Backlinks;
- New document, open workspace, close workspace/Home;
- History restore;
- Outline section moves;
- Settings open and any equivalent full-screen editor unmount.

Passive panel visibility and read-only scanning remain unprotected when they
neither consume canonical source nor unmount the source editor. Startup recovery
is an explicit exception because it is rendered before `MainShell`; this must
remain structurally unreachable while a source editor is mounted.

## 12. Internal module boundaries

The current `source_sync.rs` exceeds the repository's enforced 500-ELOC limit.
Implementation should separate responsibilities, for example:

```text
source_sync.rs                  public facade and shared exports
source_sync/state.rs            pure lifecycle and barrier state
source_sync/commands.rs         protected command execution
source_sync/controller.rs       Dioxus/eval orchestration and timeouts
source_sync/tests/              state, protocol, and command tests
../bekoedit-ui-contract/src/source_editor.rs  typed bridge protocol
```

Exact names may change, but lifecycle transition logic must remain testable
without a WebView.

Only transition methods in `state.rs` may mutate lifecycle/barrier state.
`controller.rs` feeds typed events to the reducer and executes emitted effects.
`commands.rs` executes approved post-barrier commands but cannot directly
change lifecycle state. Apply the 300-ELOC guideline and 500-ELOC hard limit to
every implementation and test file.

TextMode and SplitMode should share a source-editor host/controller hook rather
than duplicate init, refresh, snapshot, relay, and teardown effects.

## 13. Testing strategy

### 13.1 Pure lifecycle tests

- controller identity survives Text -> Preview, Text -> Split, Settings ->
  return, and workspace/session replacement;
- editor instance identity changes across each controlled replacement;
- application shutdown terminates pending work without executing its command;
- mount intent is not Ready or active;
- relay acknowledgement alone is not Ready;
- bundle readiness alone is not Ready;
- init begins only when bundle and relay are ready;
- only matching `EditorReady` creates the active editor;
- stale Ready cannot activate an old instance;
- immediate protected command waits for Ready, then requests a snapshot;
- mount failure/timeout rejects and clears the waiting command;
- unmount invalidates the instance and clears its pending request;
- destroy for an old instance cannot affect a new one;
- a new mount waits during old-instance unmount;
- matched destroy releases the single waiting mount;
- destroy timeout authorizes only the correlated one-use takeover permit;
- duplicate exact init is idempotent without reconstructing the view;
- mismatched duplicate, stale init, and unauthorized takeover are rejected;
- late drop and late destroy cannot affect the replacement instance;
- History restore and both Outline moves enter RefreshPending;
- refresh invalidates old snapshot capability before dispatch;
- matching DocumentApplied activates the new epoch and canonical revision;
- refresh no-op still rolls epoch and requires acknowledgement;
- stale refresh acknowledgement, late old-epoch debounce, and wrong revision
  cannot reactivate the editor;
- refresh during composition and refresh timeout fail closed;
- a protected command during refresh does not replace or extend it;
- every blocked/rejected/timeout path returns to a retryable non-busy state.

### 13.2 Protocol and barrier tests

- Text and Split route through their matching instance relay;
- Rust and JavaScript protocol constants both equal version 2;
- unsupported protocol versions are rejected;
- missing/wrong instance, editor, document, epoch, sequence, and request IDs
  cannot mutate or complete current state;
- no-op snapshots complete commands without revision bumps;
- ordinary debounced change racing a forced snapshot preserves ordering;
- barrier hold prevents post-snapshot input from racing command execution;
- first matching terminal event wins over duplicate result/timeout races;
- mount, snapshot, refresh, and destroy deadlines have distinct outcomes;
- composition blocks protected commands without publishing partial text;
- all existing protected commands execute only after accepted synchronization;
- Settings open synchronizes before unmount;
- startup recovery remains unreachable with an active source editor.

### 13.3 JavaScript adapter tests

- init reports missing container;
- init creates one view and returns Ready;
- duplicate exact init returns idempotent Ready without rebuilding the view;
- mismatched duplicate init is rejected;
- init for a new instance returns InstanceAlreadyActive without a valid
  takeover permit;
- a valid one-use takeover permit replaces only its named retired instance;
- requestSnapshot always returns a terminal result;
- a successful protected snapshot enters barrier hold;
- ResumeEditing and ApplyDocument release hold only after matched processing;
- ApplyDocument returns correlated DocumentApplied and installs the new epoch;
- destroy clears timers, composition, view, and identity;
- stale destroy/request cannot affect the current instance;
- relay absence produces explicit failure.

### 13.4 Desktop regression checklist

- New -> Text becomes editable without clicking Settings.
- Click Preview immediately after New; it waits or completes without error.
- Type -> immediately Preview shows the final text.
- Repeated Preview clicks do not create busy/timeout toast loops.
- Settings -> return behaves the same as the first mount.
- Type -> Settings -> return preserves typed text.
- Text -> Split and Split -> Preview preserve final text.
- Save/Save As/open another document immediately after typing preserve source.
- IME composition/commit works; active composition blocks safely.
- Run the critical sequence on Linux, macOS, and Windows release candidates.

Before release, Linux CI must include at least one repeatable regression that
launches the actual Dioxus/WebView surface and exercises New -> Ready -> type ->
immediate Preview. The existing headless logic smoke does not satisfy this
requirement. macOS and Windows retain candidate-specific manual lifecycle and
IME evidence unless equivalent automation is added.

Automated Rust tests alone are not acceptance evidence for WebView lifecycle
ordering.

## 14. Rollout plan

1. Review and approve RFC-041 before implementation.
2. Add protocol version 2 types in `bekoedit-ui-contract` and adapter contract
   tests.
3. Split the source-sync module and introduce the pure reducer and tests.
4. Host the controller/relay at app root and implement persistent bundle
   readiness.
5. Implement serialized init/destroy/takeover and relay-before-init handshake.
6. Implement Ready, snapshot hold/resume, refresh/rebase, and first-terminal
   deadline behavior.
7. Migrate Text and Split to mount/drop intent and shared rendering host logic.
8. Protect Settings and audit every source-editor unmount path.
9. Rebuild the checked-in JavaScript bundle.
10. Make app tests blocking in CI and add the Linux WebView regression.
11. Run focused tests, full repository gates, and cross-platform desktop manual
    regression.
12. Submit an implementation review package; move this RFC to `done/` only
    when the reviewed implementation ships.

## 15. Alternatives considered

### Keep adding guards to the current effects

Rejected. Guards can reduce duplicate initialization but do not prove bundle,
relay, view, or snapshot readiness and do not define teardown.

### Treat the first timeout as editor readiness detection

Rejected. A protected user action must not be the probe that discovers a fake
active editor.

### Replace CodeMirror

Rejected. The failure occurs before or around adapter initialization. No
evidence identifies CodeMirror's editor model as the cause.

### Keep separate Text and Split bridge controllers

Rejected. Only one global CodeMirror adapter/view is supported; independent
controllers recreate ambiguous ownership and duplicated lifecycle effects.

### Keep the singleton without instance identity

Rejected. A singleton is acceptable only with one explicit owner and stale
instance rejection.

## 16. Initial review resolutions and rereview questions

The 2026-07-14 initial architecture review resolved the original questions:

- bundle and instance readiness remain separate;
- immediate protected commands wait within a bounded single-flight operation;
- one singleton adapter remains under one root-surviving controller;
- Settings is protected;
- instance ID and epoch remain distinct opaque identities;
- Linux gains an actual WebView regression, while macOS/Windows retain manual
  candidate and IME evidence until equivalent automation exists.

Revision 2 requests focused confirmation that:

1. application-root hosting and separate application-exit shutdown close the
   controller lifetime gap;
2. `RefreshPending` plus `DocumentApplied` closes History/Outline stale-text
   overwrite paths;
3. matched destroy, waiting mount, and one-use takeover permit serialize every
   singleton replacement;
4. bridge schema version 2, the typed operation family, distinct deadlines,
   and first-terminal-wins semantics close compatibility and terminalization
   ambiguity;
5. barrier hold/resume prevents post-snapshot input races without weakening
   IME safety;
6. reducer-only mutation authority keeps one lifecycle owner after module
   separation.

Implementation must not begin until rereview confirms the blocking findings
are resolved.
