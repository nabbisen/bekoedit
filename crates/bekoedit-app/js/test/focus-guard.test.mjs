import assert from "node:assert/strict";
import test from "node:test";

import {
  consumeFocusRequest,
  createBrowserFocusGuardRegistry,
  createFocusGuardRegistry,
} from "../src/focus-guard.js";

function element(name) {
  return {
    name,
    isConnected: true,
    children: new Set(),
    contains(node) { return node === this || this.children.has(node); },
  };
}

function harness() {
  const body = element("body");
  const origins = new Map();
  const listeners = new Map();
  let active = body;
  const registry = createFocusGuardRegistry({
    resolveOrigin: (request) => origins.get(request.fingerprint) ?? null,
    activeElement: () => active,
    body: () => body,
    addListener(type, listener) {
      const set = listeners.get(type) ?? new Set();
      set.add(listener);
      listeners.set(type, set);
    },
    removeListener(type, listener) { listeners.get(type)?.delete(listener); },
  });
  const fire = (type, event) => {
    for (const listener of listeners.get(type) ?? []) listener(event);
  };
  return {
    registry,
    body,
    origins,
    setActive(value) { active = value; },
    fire,
    listenerCount: () => [...listeners.values()].reduce((sum, set) => sum + set.size, 0),
  };
}

const request = (token, fingerprint = `request-${token}`) => ({
  token,
  fingerprint,
  removalPolicy: "launchMayBeRemoved",
});

test("browser pointer origin does not depend on button focus", () => {
  const body = element("body");
  const launch = {
    ...element("launch"),
    dataset: { sourceFocusLaunch: "start-new" },
    getClientRects: () => [{}],
  };
  const document = {
    activeElement: body,
    body,
    querySelectorAll: (selector) => selector === "[data-source-focus-launch]" ? [launch] : [],
    addEventListener() {},
    removeEventListener() {},
  };
  const registry = createBrowserFocusGuardRegistry(document);

  assert.equal(registry.arm({
    ...request(1),
    invocation: "pointer",
    launchId: "start-new",
  }).armed, true);
});

test("timeout before delayed arm fences installation", () => {
  const h = harness();
  h.origins.set("request-1", element("old"));
  h.registry.cancelThrough(1);
  assert.equal(h.registry.arm(request(1)).armed, false);
  assert.equal(h.registry.inspect().currentToken, null);
  assert.equal(h.listenerCount(), 0);
});

test("new arm fences delayed old arm without replacing listeners", () => {
  const h = harness();
  const newer = element("newer");
  h.origins.set("request-2", newer);
  h.origins.set("request-1", element("older"));
  assert.equal(h.registry.arm(request(2)).armed, true);
  assert.equal(h.registry.arm(request(1)).armed, false);
  assert.equal(h.registry.inspect().currentToken, 2);
  assert.equal(h.listenerCount(), 3);
});

test("losing acknowledgement cleanup removes the exact old guard", () => {
  const h = harness();
  h.origins.set("request-1", element("old"));
  assert.equal(h.registry.arm(request(1)).armed, true);
  h.registry.cancelThrough(1);
  assert.equal(h.registry.inspect().currentToken, null);
  assert.equal(h.listenerCount(), 0);
});

test("delayed old cleanup cannot remove a newer guard", () => {
  const h = harness();
  h.origins.set("request-2", element("new"));
  assert.equal(h.registry.arm(request(2)).armed, true);
  h.registry.cancelThrough(1);
  assert.equal(h.registry.inspect().currentToken, 2);
  assert.equal(h.listenerCount(), 3);
});

test("invalid newer arm fences old retry and fallback removes installed old guard", () => {
  const h = harness();
  h.origins.set("request-1", element("old"));
  assert.equal(h.registry.arm(request(1)).armed, true);
  assert.equal(h.registry.arm(request(2)).armed, false);
  assert.equal(h.registry.arm(request(1)).armed, false);
  h.registry.cancelThrough(2);
  assert.equal(h.registry.inspect().currentToken, null);
  assert.equal(h.listenerCount(), 0);
});

test("same-token retries cache both positive and negative outcomes", () => {
  const positive = harness();
  positive.origins.set("same", element("origin"));
  assert.equal(positive.registry.arm(request(1, "same")).armed, true);
  assert.equal(positive.registry.arm(request(1, "same")).armed, true);
  assert.equal(positive.registry.arm(request(1, "different")).armed, false);
  assert.equal(positive.listenerCount(), 3);

  const negative = harness();
  assert.equal(negative.registry.arm(request(1, "same")).armed, false);
  negative.origins.set("same", element("late-origin"));
  assert.equal(negative.registry.arm(request(1, "same")).armed, false);
  assert.equal(negative.listenerCount(), 0);
});

test("diversion is irreversible and removable body fallback is one-use", () => {
  const h = harness();
  const origin = element("origin");
  const other = element("other");
  h.origins.set("request-1", origin);
  h.setActive(origin);
  assert.equal(h.registry.arm(request(1)).armed, true);
  h.fire("focusin", { target: other });
  origin.isConnected = false;
  h.setActive(h.body);
  assert.equal(h.registry.consume(1, "request-1"), false);
  assert.equal(h.registry.consume(1, "request-1"), false);
  assert.equal(h.listenerCount(), 0);

  const natural = harness();
  const removable = element("removable");
  natural.origins.set("request-1", removable);
  natural.setActive(removable);
  assert.equal(natural.registry.arm(request(1)).armed, true);
  removable.isConnected = false;
  natural.setActive(natural.body);
  natural.fire("focusin", { target: natural.body });
  assert.equal(natural.registry.consume(1, "request-1"), true);
  assert.equal(natural.registry.consume(1, "request-1"), false);
});

test("origin descendants remain valid while Tab and outside pointer divert", () => {
  const valid = harness();
  const origin = element("origin");
  const child = element("svg-child");
  origin.children.add(child);
  valid.origins.set("request-1", origin);
  valid.setActive(child);
  assert.equal(valid.registry.arm(request(1)).armed, true);
  valid.fire("pointerdown", { target: child });
  assert.equal(valid.registry.consume(1, "request-1"), true);

  for (const event of [
    ["keydown", { key: "Tab" }],
    ["pointerdown", { target: element("outside") }],
  ]) {
    const diverted = harness();
    const launch = element("launch");
    diverted.origins.set("request-1", launch);
    diverted.setActive(launch);
    assert.equal(diverted.registry.arm(request(1)).armed, true);
    diverted.fire(event[0], event[1]);
    assert.equal(diverted.registry.consume(1, "request-1"), false);
  }
});

test("persistent origins must remain connected", () => {
  const h = harness();
  const origin = element("persistent");
  h.origins.set("persistent", origin);
  h.setActive(origin);
  const persistent = {
    ...request(1, "persistent"),
    removalPolicy: "launchMustRemain",
  };
  assert.equal(h.registry.arm(persistent).armed, true);
  origin.isConnected = false;
  h.setActive(h.body);
  assert.equal(h.registry.consume(1, "persistent"), false);
  assert.equal(h.listenerCount(), 0);
});

test("only an exact successful consumption emits the success trace", () => {
  const identity = {
    instanceId: 4,
    editorId: "text",
    documentId: 7,
    epoch: 2,
  };
  const run = ({ actualIdentity = identity, divert = false } = {}) => {
    const h = harness();
    const origin = element("origin");
    h.origins.set("request-1", origin);
    h.setActive(origin);
    h.registry.arm(request(1));
    if (divert) h.fire("keydown", { key: "Tab" });
    const traces = [];
    let focuses = 0;
    const accepted = consumeFocusRequest(
      h.registry,
      { token: 1, fingerprint: "request-1", identity },
      actualIdentity,
      () => { focuses += 1; },
      (name) => traces.push(name),
    );
    return { accepted, focuses, traces };
  };

  assert.deepEqual(run(), {
    accepted: true,
    focuses: 1,
    traces: ["source.focus.consumed"],
  });
  assert.deepEqual(run({ divert: true }), {
    accepted: false,
    focuses: 0,
    traces: ["source.focus.rejected.guard"],
  });
  assert.deepEqual(run({ actualIdentity: { ...identity, epoch: 3 } }), {
    accepted: false,
    focuses: 0,
    traces: ["source.focus.rejected.identity"],
  });
});
