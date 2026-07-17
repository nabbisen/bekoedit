import assert from "node:assert/strict";
import test from "node:test";

import {
  FOCUS_GUARD_PROTOCOL_VERSION,
  consumeFocusRequest,
  createBrowserFocusGuardRegistry,
  createFocusGuardRegistry,
  installBrowserFocusGuardRegistry,
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

test("browser registry installation is idempotent and preserves live state", () => {
  const body = element("body");
  const launch = {
    ...element("launch"),
    dataset: { sourceFocusLaunch: "start-new" },
    getClientRects: () => [{}],
  };
  const document = {
    activeElement: body,
    body,
    querySelectorAll: () => [launch],
    addEventListener() {},
    removeEventListener() {},
  };
  const globalObject = {};
  const first = installBrowserFocusGuardRegistry(globalObject, document);
  assert.equal(first.protocolVersion, FOCUS_GUARD_PROTOCOL_VERSION);
  assert.equal(first.arm({
    ...request(7),
    invocation: "pointer",
    launchId: "start-new",
  }).armed, true);

  const second = installBrowserFocusGuardRegistry(globalObject, document);
  assert.equal(second, first);
  assert.equal(second.inspect().currentToken, 7);
});

test("browser registry installation fails closed on an incompatible owner", () => {
  const incompatible = { protocolVersion: FOCUS_GUARD_PROTOCOL_VERSION + 1 };
  const globalObject = { __bkFocusGuards: incompatible };
  assert.equal(installBrowserFocusGuardRegistry(globalObject, {}), null);
  assert.equal(globalObject.__bkFocusGuards, incompatible);
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

test("diagnostics distinguish missing guard and token mismatch without exposing current token", () => {
  const missing = harness();
  assert.deepEqual(missing.registry.consumeDiagnostic(1, "secret-fingerprint"), {
    accepted: false,
    diagnostic: {
      outcome: "rejected",
      reason: "missingGuard",
      tokenRelation: "noGuard",
      diversion: "notEvaluated",
      fingerprintRelation: "notEvaluated",
      originConnection: "notEvaluated",
      activeElementRelation: "notEvaluated",
      removalPolicy: "notEvaluated",
      removedBodyFallback: "notEvaluated",
    },
  });

  const mismatch = harness();
  const newer = element("newer-secret-element");
  mismatch.origins.set("newer-secret-fingerprint", newer);
  mismatch.setActive(newer);
  mismatch.registry.arm(request(2, "newer-secret-fingerprint"));
  const result = mismatch.registry.consumeDiagnostic(1, "older-secret-fingerprint");
  assert.equal(result.accepted, false);
  assert.equal(result.diagnostic.reason, "tokenMismatch");
  assert.equal(result.diagnostic.tokenRelation, "mismatch");
  assert.equal(JSON.stringify(result).includes("newer-secret"), false);
  assert.equal(mismatch.registry.inspect().currentToken, 2);
  assert.equal(mismatch.listenerCount(), 3);
});

test("first diversion source is irreversible and independently reports fingerprint state", () => {
  const cases = [
    {
      fire: (h, origin) => h.fire("pointerdown", { target: element("outside") }),
      later: (h) => h.fire("keydown", { key: "Tab" }),
      reason: "divertedPointer",
      diversion: "pointer",
    },
    {
      fire: (h) => h.fire("keydown", { key: "Tab" }),
      later: (h) => h.fire("focusin", { target: element("outside") }),
      reason: "divertedTab",
      diversion: "tab",
    },
    {
      fire: (h) => h.fire("focusin", { target: element("outside") }),
      later: (h) => h.fire("pointerdown", { target: element("outside") }),
      reason: "divertedFocusIn",
      diversion: "focusIn",
    },
  ];
  for (const entry of cases) {
    const h = harness();
    const origin = element("origin");
    h.origins.set("request-1", origin);
    h.setActive(origin);
    assert.equal(h.registry.arm(request(1)).armed, true);
    entry.fire(h, origin);
    entry.later(h);
    const result = h.registry.consumeDiagnostic(1, "different-fingerprint");
    assert.equal(result.accepted, false);
    assert.equal(result.diagnostic.reason, entry.reason);
    assert.equal(result.diagnostic.diversion, entry.diversion);
    assert.equal(result.diagnostic.fingerprintRelation, "mismatch");
    assert.equal(h.listenerCount(), 0);
    assert.equal(h.registry.consumeDiagnostic(1, "request-1").diagnostic.reason, "missingGuard");
  }
});

test("diagnostics classify every origin and active-element eligibility state", () => {
  const cases = [
    ["connected-origin", true, "origin", "launchMayBeRemoved", true, "ineligible"],
    ["connected-body", true, "body", "launchMayBeRemoved", false, "ineligible"],
    ["connected-other", true, "other", "launchMayBeRemoved", false, "ineligible"],
    ["connected-none", true, "none", "launchMayBeRemoved", false, "ineligible"],
    ["removed-body-allowed", false, "body", "launchMayBeRemoved", true, "eligible"],
    ["removed-body-forbidden", false, "body", "launchMustRemain", false, "ineligible"],
    ["removed-origin", false, "origin", "launchMayBeRemoved", false, "ineligible"],
    ["removed-other", false, "other", "launchMayBeRemoved", false, "ineligible"],
    ["removed-none", false, "none", "launchMayBeRemoved", false, "ineligible"],
  ];
  for (const [name, connected, relation, removalPolicy, accepted, fallback] of cases) {
    const h = harness();
    const origin = element(`origin-${name}`);
    const other = element(`other-${name}`);
    h.origins.set(name, origin);
    h.setActive(relation === "origin"
      ? origin
      : relation === "body"
        ? h.body
        : relation === "other"
          ? other
          : null);
    assert.equal(h.registry.arm({ ...request(1, name), removalPolicy }).armed, true);
    origin.isConnected = connected;
    const result = h.registry.consumeDiagnostic(1, name);
    assert.equal(result.accepted, accepted, name);
    assert.equal(result.diagnostic.reason, accepted ? "accepted" : "activeElementIneligible");
    assert.equal(result.diagnostic.originConnection, connected ? "connected" : "disconnected");
    assert.equal(result.diagnostic.activeElementRelation, relation);
    assert.equal(result.diagnostic.removedBodyFallback, fallback);
    assert.equal(result.diagnostic.removalPolicy, removalPolicy);
  }
});

test("fingerprint diagnostics expose equality only and legacy consume shares one-use authority", () => {
  const h = harness();
  const origin = element("private-origin-name");
  h.origins.set("private-original-fingerprint", origin);
  h.setActive(origin);
  h.registry.arm(request(1, "private-original-fingerprint"));
  const mismatch = h.registry.consumeDiagnostic(1, "private-other-fingerprint");
  assert.equal(mismatch.diagnostic.reason, "fingerprintMismatch");
  assert.equal(mismatch.diagnostic.fingerprintRelation, "mismatch");
  assert.equal(JSON.stringify(mismatch).includes("private-"), false);

  const legacy = harness();
  const legacyOrigin = element("legacy");
  legacy.origins.set("request-1", legacyOrigin);
  legacy.setActive(legacyOrigin);
  legacy.registry.arm(request(1));
  assert.equal(legacy.registry.consume(1, "request-1"), true);
  assert.equal(legacy.registry.consume(1, "request-1"), false);
});

test("wrapper traces correlate safe diagnostics and preserve identity and cancellation fences", () => {
  const identity = {
    instanceId: 4,
    editorId: "text",
    documentId: 7,
    epoch: 2,
  };
  const run = ({ suppliedRequest, actualIdentity = identity, armToken = 1 } = {}) => {
    const h = harness();
    const origin = element("private-origin");
    h.origins.set(`request-${armToken}`, origin);
    h.setActive(origin);
    h.registry.arm(request(armToken));
    const traces = [];
    let focuses = 0;
    const accepted = consumeFocusRequest(
      h.registry,
      suppliedRequest,
      actualIdentity,
      () => { focuses += 1; },
      (name, details) => traces.push({ name, details }),
    );
    return { accepted, focuses, traces, h };
  };

  const exactRequest = { token: 1, fingerprint: "request-1", identity };
  const accepted = run({ suppliedRequest: exactRequest });
  assert.equal(accepted.accepted, true);
  assert.equal(accepted.focuses, 1);
  assert.equal(accepted.traces[0].name, "source.focus.consumed");
  assert.equal(accepted.traces[0].details.focusToken, 1);
  assert.equal(accepted.traces[0].details.focusGuardDiagnostic.reason, "accepted");

  const missingRequest = run({ suppliedRequest: null });
  assert.equal(missingRequest.accepted, false);
  assert.equal(missingRequest.traces[0].details.focusToken, null);
  assert.equal(
    missingRequest.traces[0].details.focusGuardDiagnostic.reason,
    "requestMissing",
  );

  const identityMismatch = run({
    suppliedRequest: exactRequest,
    actualIdentity: { ...identity, epoch: 3 },
  });
  assert.equal(identityMismatch.accepted, false);
  assert.equal(
    identityMismatch.traces[0].details.focusGuardDiagnostic.reason,
    "identityMismatch",
  );

  const newer = run({
    suppliedRequest: exactRequest,
    armToken: 2,
  });
  assert.equal(newer.accepted, false);
  assert.equal(newer.traces[0].details.focusGuardDiagnostic.reason, "tokenMismatch");
  assert.equal(newer.h.registry.inspect().currentToken, 2);
  assert.equal(newer.h.listenerCount(), 3);

  const older = run({
    suppliedRequest: { token: 2, fingerprint: "request-2", identity },
    armToken: 1,
  });
  assert.equal(older.accepted, false);
  assert.equal(older.traces[0].details.focusGuardDiagnostic.reason, "tokenMismatch");
  assert.equal(older.h.registry.inspect().currentToken, null);
  assert.equal(older.h.listenerCount(), 0);
});

test("trace delivery failure cannot reverse an accepted or rejected focus decision", () => {
  const identity = {
    instanceId: 4,
    editorId: "text",
    documentId: 7,
    epoch: 2,
  };
  const accepted = harness();
  const origin = element("origin");
  accepted.origins.set("request-1", origin);
  accepted.setActive(origin);
  accepted.registry.arm(request(1));
  let focuses = 0;
  assert.equal(consumeFocusRequest(
    accepted.registry,
    { token: 1, fingerprint: "request-1", identity },
    identity,
    () => { focuses += 1; },
    () => { throw new Error("trace unavailable"); },
  ), true);
  assert.equal(focuses, 1);

  const rejected = harness();
  assert.equal(consumeFocusRequest(
    rejected.registry,
    { token: 1, fingerprint: "request-1", identity },
    identity,
    () => { focuses += 1; },
    () => { throw new Error("trace unavailable"); },
  ), false);
  assert.equal(focuses, 1);
});
