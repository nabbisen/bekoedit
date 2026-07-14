import assert from "node:assert/strict";
import test from "node:test";

import {
  BRIDGE_SCHEMA_VERSION,
  createLifecycleAdapter,
} from "../src/lifecycle.js";

const identity = (instanceId = 1, epoch = 1) => ({
  instanceId,
  editorId: "text",
  documentId: 7,
  epoch,
});

function request(type, operationId, fields = {}) {
  return {
    type,
    protocolVersion: BRIDGE_SCHEMA_VERSION,
    operationId,
    ...fields,
  };
}

function harness({ container = {} } = {}) {
  const events = [];
  let creates = 0;
  let destroys = 0;
  let cancels = 0;
  const adapter = createLifecycleAdapter({
    emit(payload) {
      events.push(payload);
      return true;
    },
    getContainer: () => container,
    createView(_parent, text, rejectsChange) {
      creates += 1;
      return { text, rejectsChange };
    },
    destroyView() { destroys += 1; },
    getText: (view) => view.text,
    replaceDocument(view, text) { view.text = text; },
    focusView() {},
    cancelPendingChange() { cancels += 1; },
  });
  return {
    adapter,
    events,
    counts: () => ({ creates, destroys, cancels }),
  };
}

function init(adapter, operationId = 1, currentIdentity = identity(), takeover = null) {
  adapter.dispatch(request("initEditor", operationId, {
    identity: currentIdentity,
    containerId: "cm-root",
    revision: 3,
    text: "initial",
    takeover,
  }));
}

test("protocol authority is version two", () => {
  assert.equal(BRIDGE_SCHEMA_VERSION, 2);
});

test("probe, relay, and missing-container init always terminalize", () => {
  const { adapter, events } = harness({ container: null });
  adapter.dispatch(request("probeBundle", 1));
  adapter.dispatch(request("installRelay", 2, { identity: identity() }));
  init(adapter, 3);
  assert.deepEqual(events.map((item) => item.type), [
    "bundleReady",
    "relayReady",
    "initFailed",
  ]);
  assert.equal(events[2].reason, "missingContainer");
});

test("exact duplicate init is reused and mismatched duplicate is rejected", () => {
  const { adapter, events, counts } = harness();
  init(adapter, 1);
  init(adapter, 2);
  init(adapter, 3, { ...identity(), epoch: 9 });
  assert.equal(counts().creates, 1);
  assert.equal(events[0].type, "editorReady");
  assert.equal(events[0].reused, false);
  assert.equal(events[1].type, "editorReady");
  assert.equal(events[1].reused, true);
  assert.equal(events[2].type, "initFailed");
  assert.equal(events[2].reason, "identityMismatch");
});

test("new singleton ownership requires a correlated one-use takeover", () => {
  const { adapter, events, counts } = harness();
  init(adapter, 1);
  init(adapter, 2, identity(2, 2));
  assert.equal(events.at(-1).reason, "instanceAlreadyActive");
  init(adapter, 3, identity(2, 2), {
    retiredInstanceId: 1,
    replacementInstanceId: 2,
    nonce: 44,
  });
  assert.equal(events.at(-1).type, "editorReady");
  assert.deepEqual(counts(), { creates: 2, destroys: 1, cancels: 1 });
  init(adapter, 4, identity(3, 3), {
    retiredInstanceId: 2,
    replacementInstanceId: 3,
    nonce: 44,
  });
  assert.equal(events.at(-1).reason, "instanceAlreadyActive");
});

test("snapshot enters hold and matched resume is idempotent", () => {
  const { adapter, events } = harness();
  init(adapter);
  adapter.dispatch(request("requestSnapshot", 2, { identity: identity() }));
  assert.equal(events.at(-1).type, "snapshot");
  assert.equal(adapter.isHeld(), true);
  assert.equal(adapter.publishChange("racing text"), false);
  const resume = request("resumeEditing", 3, {
    identity: identity(),
    snapshotOperationId: 2,
    revision: 3,
  });
  adapter.dispatch(resume);
  adapter.dispatch(resume);
  assert.equal(adapter.isHeld(), false);
  assert.equal(events.at(-2).type, "editingResumed");
  assert.deepEqual(events.at(-1), events.at(-2));
});

test("resume safely acknowledges a snapshot request lost before hold", () => {
  const { adapter, events } = harness();
  init(adapter);
  adapter.dispatch(request("resumeEditing", 3, {
    identity: identity(),
    snapshotOperationId: 2,
    revision: 3,
  }));
  assert.equal(events.at(-1).type, "editingResumed");
  assert.equal(events.at(-1).wasHeld, false);
  assert.equal(adapter.isHeld(), false);
});

test("resume rejects an older hold after a newer hold was released", () => {
  const { adapter, events } = harness();
  init(adapter);
  adapter.dispatch(request("requestSnapshot", 5, { identity: identity() }));
  adapter.dispatch(request("resumeEditing", 6, {
    identity: identity(),
    snapshotOperationId: 5,
    revision: 3,
  }));
  adapter.dispatch(request("resumeEditing", 7, {
    identity: identity(),
    snapshotOperationId: 4,
    revision: 3,
  }));
  assert.equal(events.at(-1).type, "resumeFailed");
  assert.equal(events.at(-1).reason, "identityMismatch");
  assert.equal(adapter.isHeld(), false);
});

test("composition blocks snapshot without entering hold", () => {
  const { adapter, events } = harness();
  init(adapter);
  adapter.compositionStarted();
  adapter.dispatch(request("requestSnapshot", 2, { identity: identity() }));
  assert.equal(events.at(-1).type, "snapshotBlocked");
  assert.equal(events.at(-1).reason, "compositionActive");
  assert.equal(adapter.isHeld(), false);
});

test("ApplyDocument releases hold and acknowledges the fresh epoch", () => {
  const { adapter, events } = harness();
  init(adapter);
  adapter.dispatch(request("requestSnapshot", 2, { identity: identity() }));
  adapter.dispatch(request("applyDocument", 3, {
    oldIdentity: identity(),
    newEpoch: 8,
    revision: 4,
    text: "canonical",
  }));
  assert.equal(events.at(-1).type, "documentApplied");
  assert.deepEqual(events.at(-1).identity, identity(1, 8));
  assert.equal(events.at(-1).revision, 4);
  assert.equal(adapter.isHeld(), false);
});

test("stale destroy cannot affect the live instance", () => {
  const { adapter, events, counts } = harness();
  init(adapter);
  adapter.dispatch(request("destroyEditor", 2, { identity: identity(9, 9) }));
  assert.equal(events.at(-1).type, "destroyFailed");
  assert.equal(counts().destroys, 0);
  adapter.dispatch(request("destroyEditor", 3, { identity: identity() }));
  adapter.dispatch(request("destroyEditor", 4, { identity: identity() }));
  assert.equal(counts().destroys, 1);
  assert.equal(events.at(-1).type, "destroyed");
});

test("unsupported versions fail without editor mutation", () => {
  const { adapter, events, counts } = harness();
  adapter.dispatch({
    ...request("initEditor", 1, {
      identity: identity(),
      containerId: "cm-root",
      revision: 3,
      text: "initial",
    }),
    protocolVersion: 1,
  });
  assert.equal(events.at(-1).reason, "unsupportedVersion");
  assert.equal(counts().creates, 0);
});
