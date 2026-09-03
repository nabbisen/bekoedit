import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  FakeDioxus,
  acknowledgement,
  request as requestPhase,
  runDriver as runDriverWith,
} from "./webview-smoke-driver-harness.mjs";

const driverSource = readFileSync(
  new URL("../../src/webview_smoke/driver.js", import.meta.url),
  "utf8",
);

function installBrowser({ beforeDomRead = () => {} } = {}) {
  globalThis.window = {};
  globalThis.Node = { ELEMENT_NODE: 1 };
  globalThis.MouseEvent = class MouseEvent {};
  globalThis.MutationObserver = class MutationObserver {
    observe() {}
    disconnect() {}
  };
  globalThis.document = {
    documentElement: {},
    querySelector() {
      beforeDomRead();
      return null;
    },
  };
}

// This file only ever exercises the launch phase (querySelector stubbed to
// null, so every run takes the "element not found -> pending" branch); the
// phases test file drives editor/preview. `request` still takes a phase
// argument -- shared with that file per RFC-044 slice-1 handoff §8 -- so
// this local wrapper just pins it to "launch".
function request(exchangeId, release = null) {
  return requestPhase(exchangeId, "launch", release);
}

function runDriver(dioxus) {
  return runDriverWith(driverSource, dioxus);
}

test("channel is pinned before typed return and survives close", { concurrency: false }, async () => {
  installBrowser();
  const dioxus = new FakeDioxus();
  const completionPromise = runDriver(dioxus);
  dioxus.push(request(1));
  const report = await dioxus.nextSent();
  assert.deepEqual(
    {
      protocolVersion: report.protocolVersion,
      exchangeId: report.exchangeId,
      phase: report.phase,
      kind: report.kind,
      releasedExchangeId: report.releasedExchangeId,
      releasedPhase: report.releasedPhase,
    },
    {
      protocolVersion: 2,
      exchangeId: 1,
      phase: "launch",
      kind: "pending",
      releasedExchangeId: null,
      releasedPhase: null,
    },
  );
  dioxus.push(acknowledgement(report));
  const completion = await completionPromise;
  assert.equal(completion.acknowledgementProcessed, true);
  assert.equal(completion.evaluatorPinned, true);
  assert.equal(window.__bkWebViewSmokeEvalPin.current.channel, dioxus);

  dioxus.close();
  assert.equal(dioxus.closed, true);
  assert.equal(
    window.__bkWebViewSmokeEvalPin.current.channel,
    dioxus,
    "close does not remove the explicit strong pin",
  );
});

test("next probe releases the exact prior pin before DOM access", { concurrency: false }, async () => {
  installBrowser();
  const first = new FakeDioxus();
  const firstCompletion = runDriver(first);
  first.push(request(10));
  const firstReport = await first.nextSent();
  first.push(acknowledgement(firstReport));
  await firstCompletion;

  const second = new FakeDioxus();
  let domReads = 0;
  document.querySelector = () => {
    assert.equal(window.__bkWebViewSmokeEvalPin.current, null);
    domReads += 1;
    return null;
  };
  const secondCompletion = runDriver(second);
  second.push(request(11, { exchangeId: 10, phase: "launch" }));
  const secondReport = await second.nextSent();
  assert.equal(secondReport.releasedExchangeId, 10);
  assert.equal(secondReport.releasedPhase, "launch");
  assert.ok(domReads > 0);
  second.push(acknowledgement(secondReport));
  await secondCompletion;
  assert.equal(window.__bkWebViewSmokeEvalPin.current.exchangeId, 11);
  assert.equal(window.__bkWebViewSmokeEvalPin.current.channel, second);
});

test("wrong acknowledgement rejects without creating a pin", { concurrency: false }, async () => {
  installBrowser();
  const dioxus = new FakeDioxus();
  const completion = runDriver(dioxus);
  dioxus.push(request(20));
  const report = await dioxus.nextSent();
  dioxus.push({ ...acknowledgement(report), exchangeId: 21 });
  await assert.rejects(completion, /invalid phase acknowledgement/);
  assert.equal(window.__bkWebViewSmokeEvalPin.current, null);
});

test("missing acknowledgement leaves the promise pending and unpinned", { concurrency: false }, async () => {
  installBrowser();
  const dioxus = new FakeDioxus();
  const completion = runDriver(dioxus);
  dioxus.push(request(30));
  await dioxus.nextSent();
  const stillPending = await Promise.race([
    completion.then(
      () => false,
      () => false,
    ),
    new Promise((resolve) => setImmediate(() => resolve(true))),
  ]);
  assert.equal(stillPending, true);
  assert.equal(window.__bkWebViewSmokeEvalPin.current, null);
});

test("incompatible or mismatched pin state fails before DOM work", { concurrency: false }, async () => {
  installBrowser({
    beforeDomRead() {
      assert.fail("invalid pin state must fail before DOM access");
    },
  });
  window.__bkWebViewSmokeEvalPin = { protocolVersion: 99, current: null };
  const incompatible = new FakeDioxus();
  const incompatibleRun = runDriver(incompatible);
  incompatible.push(request(40));
  await assert.rejects(incompatibleRun, /incompatible smoke evaluator pin registry/);

  installBrowser();
  const first = new FakeDioxus();
  const firstRun = runDriver(first);
  first.push(request(41));
  const report = await first.nextSent();
  first.push(acknowledgement(report));
  await firstRun;
  document.querySelector = () => {
    assert.fail("mismatched release must fail before DOM access");
  };
  const mismatch = new FakeDioxus();
  const mismatchRun = runDriver(mismatch);
  mismatch.push(request(42, { exchangeId: 999, phase: "launch" }));
  await assert.rejects(mismatchRun, /prior evaluator pin did not match release request/);
  assert.equal(window.__bkWebViewSmokeEvalPin.current.exchangeId, 41);
});
