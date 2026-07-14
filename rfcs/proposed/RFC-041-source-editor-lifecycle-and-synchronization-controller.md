# RFC-041: Source Editor Lifecycle and Synchronization Controller

**Project:** bekoedit
**Status:** Proposed
**Track:** Source safety / WebView lifecycle
**Priority:** Critical
**Date:** 2026-07-14
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

### 6.3 Component responsibilities

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
7. JavaScript validates the container, destroys any explicitly stale singleton
   view, constructs exactly one view, then emits `EditorReady`.
8. Rust validates the identity and transitions to `Ready`.
9. The controller may focus the new editor after Ready.

Initialization failure transitions to `Unavailable`, clears any queued
protected command with visible failure, and leaves a retry path. It must not
create an active editor record.

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
  "protocolVersion": 1,
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
  "protocolVersion": 1,
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

### 7.5 Unmount and destroy

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

## 10. Protocol rules

- Every message carries `protocolVersion` and the full applicable identity.
- Rust rejects unsupported versions before state mutation.
- JavaScript reports missing containers and missing views explicitly.
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
source_sync/protocol.rs         typed Rust/JS messages and validation
source_sync/commands.rs         protected command execution
source_sync/controller.rs       Dioxus/eval orchestration and timeouts
source_sync/tests/              state, protocol, and command tests
```

Exact names may change, but lifecycle transition logic must remain testable
without a WebView.

TextMode and SplitMode should share a source-editor host/controller hook rather
than duplicate init, refresh, snapshot, relay, and teardown effects.

## 13. Testing strategy

### 13.1 Pure lifecycle tests

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
- every blocked/rejected/timeout path returns to a retryable non-busy state.

### 13.2 Protocol and barrier tests

- Text and Split route through their matching instance relay;
- unsupported protocol versions are rejected;
- missing/wrong instance, editor, document, epoch, sequence, and request IDs
  cannot mutate or complete current state;
- no-op snapshots complete commands without revision bumps;
- ordinary debounced change racing a forced snapshot preserves ordering;
- composition blocks protected commands without publishing partial text;
- all existing protected commands execute only after accepted synchronization;
- Settings open synchronizes before unmount;
- startup recovery remains unreachable with an active source editor.

### 13.3 JavaScript adapter tests

- init reports missing container;
- init creates one view and returns Ready;
- duplicate init for the same instance is idempotent or explicitly rejected;
- init for a new instance destroys the old instance through defined rules;
- requestSnapshot always returns a terminal result;
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

Automated Rust tests alone are not acceptance evidence for WebView lifecycle
ordering.

## 14. Rollout plan

1. Review and approve RFC-041 before implementation.
2. Split the source-sync module and introduce pure lifecycle types/tests.
3. Move bundle ownership to app scope and implement persistent boot readiness.
4. Implement the single controller and relay-before-init handshake.
5. Implement explicit Ready, snapshot, destroy, and failure protocol.
6. Migrate Text and Split to the shared host/controller boundary.
7. Protect Settings and audit every source-editor unmount path.
8. Rebuild the checked-in JavaScript bundle.
9. Run focused tests, full repository gates, and desktop manual regression.
10. Submit an implementation review package; move this RFC to `done/` only
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

## 16. Review questions

1. Is the separation between app-level bundle readiness and instance-level
   editor readiness sufficient and minimal?
2. Should an immediate protected command wait for mounting as proposed, or fail
   immediately with an editor-loading message?
3. Is one Rust-owned controller over a single JavaScript adapter preferable to
   introducing independently allocated JavaScript adapter objects?
4. Is Settings correctly classified as a protected command?
5. Are instance ID plus epoch both justified, or should they be represented by
   one typed generation identity?
6. What minimum desktop automation is feasible for the New -> Text -> Preview
   regression before release?

Implementation must not begin until the blocking answers are resolved in
review.
