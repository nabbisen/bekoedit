// Task 061: the lifecycle smoke says why its editor phase timed out.
//
// driver.js waits for five conditions at once (the view exists, is connected
// and has focus; the text host exists and carries no status marker). Its
// timeout used to say only "timed out at editor_ready_focused". These tests
// run the real driver against the shared DOM fake, let the editor phase reach
// its deadline with exactly one condition missing, and read the diagnosis it
// attaches to the terminal failure.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { FakeDom, FakeElement, FakeEditorView } from "./webview-smoke-dom-fake.mjs";
import {
  FakeDioxus,
  acknowledgement,
  request,
  runDriver,
} from "./webview-smoke-driver-harness.mjs";

const driverSource = readFileSync(
  new URL("../../src/webview_smoke/driver.js", import.meta.url),
  "utf8",
);

async function exchange(phase, exchangeId, release = null) {
  const dioxus = new FakeDioxus();
  const completion = runDriver(driverSource, dioxus);
  dioxus.push(request(exchangeId, phase, release));
  const report = await dioxus.nextSent();
  dioxus.push(acknowledgement(report));
  await completion;
  return report;
}

const TEXT_HOST = '[data-source-focus-launch-region="text"]';
const STATUS = ".source-editor-status";

/** A page where the editor is fully ready, then `spoil` breaks one condition. */
function pageWith(spoil) {
  const dom = new FakeDom();
  dom.install();
  dom.setTime(1000);
  dom.setElement('[data-source-focus-launch="start-new"]', new FakeElement());
  const page = {
    dom,
    view: new FakeEditorView({ connected: true, hasFocus: true, docLength: 3 }),
    host: new FakeElement(),
  };
  spoil?.(page);
  return page;
}

function install(page) {
  page.dom.setEditorView(page.view);
  if (page.host) page.dom.setElement(TEXT_HOST, page.host);
}

/** Launches at t=1000 (deadline 16000), lets the editor phase poll once at
 * t=2500, and times it out at t=16000. Returns the terminal report. */
async function timeOut(page) {
  await exchange("launch", 1);
  page.dom.setTime(2500);
  install(page);
  const pending = await exchange("editor", 2, { exchangeId: 1, phase: "launch" });
  assert.equal(pending.kind, "pending");
  page.dom.setTime(16000);
  return exchange("editor", 3, { exchangeId: 2, phase: "editor" });
}

function conditionsLine(report) {
  const line = report.result.diagnosis.find((l) => l.startsWith("editor conditions:"));
  assert.ok(line, `no conditions line in ${JSON.stringify(report.result.diagnosis)}`);
  return line;
}

const CASES = [
  {
    name: "the view does not exist",
    spoil: (page) => (page.view = undefined),
    expected:
      "editor conditions: window.__bk._view=false view.dom.isConnected=n/a view.hasFocus=n/a " +
      "textHost=true hostHasNoStatusMarker=true",
  },
  {
    name: "the view is not connected",
    spoil: (page) => (page.view = new FakeEditorView({ connected: false, hasFocus: true })),
    expected:
      "editor conditions: window.__bk._view=true view.dom.isConnected=false view.hasFocus=true " +
      "textHost=true hostHasNoStatusMarker=true",
  },
  {
    name: "the view has no focus",
    spoil: (page) => (page.view = new FakeEditorView({ connected: true, hasFocus: false })),
    expected:
      "editor conditions: window.__bk._view=true view.dom.isConnected=true view.hasFocus=false " +
      "textHost=true hostHasNoStatusMarker=true",
  },
  {
    name: "the text host does not exist",
    spoil: (page) => (page.host = null),
    expected:
      "editor conditions: window.__bk._view=true view.dom.isConnected=true view.hasFocus=true " +
      "textHost=false hostHasNoStatusMarker=n/a",
  },
  {
    name: "the host carries a status marker",
    spoil: (page) =>
      page.host.withChild(
        STATUS,
        new FakeElement({ className: "empty-hint source-editor-status", textContent: "Loading" }),
      ),
    expected:
      "editor conditions: window.__bk._view=true view.dom.isConnected=true view.hasFocus=true " +
      "textHost=true hostHasNoStatusMarker=false",
  },
];

for (const { name, spoil, expected } of CASES) {
  test(
    `editor timeout, only this missing: ${name}`,
    { concurrency: false },
    async () => {
      const report = await timeOut(pageWith(spoil));

      assert.equal(report.kind, "terminal");
      assert.equal(report.result.ok, false);
      assert.equal(report.result.stage, "editor_ready_focused");
      assert.equal(report.result.error, "Error: timed out at editor_ready_focused");
      assert.equal(conditionsLine(report), expected);
    },
  );
}

test(
  "editor timeout: the diagnosis names the active element by tag, id, focus labels and classes, never text",
  { concurrency: false },
  async () => {
    const page = pageWith((p) => (p.view = new FakeEditorView({ hasFocus: false })));
    page.dom.setDocumentHasFocus(false);
    page.dom.setActiveElement(
      new FakeElement({
        tagName: "BUTTON",
        id: "mode-switch",
        className: "mode-tab active",
        textContent: "SECRET DOCUMENT TEXT",
      })
        .withAttribute("data-source-focus-launch", "mode-preview")
        .withAttribute("aria-label", "SECRET LABEL"),
    );

    const report = await timeOut(page);
    const diagnosis = report.result.diagnosis;

    assert.ok(
      diagnosis.includes(
        "document.hasFocus()=false; activeElement=button#mode-switch" +
          "[data-source-focus-launch=mode-preview].mode-tab.active",
      ),
      JSON.stringify(diagnosis),
    );
    assert.doesNotMatch(JSON.stringify(diagnosis), /SECRET/);
  },
);

test(
  "editor timeout: no active element is said plainly",
  { concurrency: false },
  async () => {
    const report = await timeOut(pageWith((p) => (p.view = undefined)));
    assert.ok(
      report.result.diagnosis.includes("document.hasFocus()=true; activeElement=none"),
      JSON.stringify(report.result.diagnosis),
    );
  },
);

test(
  "editor timeout: the status marker is reported by its classes, never its text",
  { concurrency: false },
  async () => {
    const page = pageWith((p) =>
      p.host.withChild(
        STATUS,
        new FakeElement({
          className: "empty-hint source-editor-status unavailable",
          textContent: "SECRET STATUS TEXT",
        }),
      ),
    );

    const report = await timeOut(page);

    assert.ok(
      report.result.diagnosis.includes(
        "statusMarker=.empty-hint.source-editor-status.unavailable",
      ),
      JSON.stringify(report.result.diagnosis),
    );
    assert.doesNotMatch(JSON.stringify(report.result.diagnosis), /SECRET/);
  },
);

test(
  "editor timeout: no status marker is said plainly",
  { concurrency: false },
  async () => {
    const report = await timeOut(pageWith((p) => (p.view = undefined)));
    assert.ok(report.result.diagnosis.includes("statusMarker=none"));
  },
);

test(
  "editor timeout: timing runs from the New click, and says when the view was first seen",
  { concurrency: false },
  async () => {
    const page = pageWith((p) => (p.view = new FakeEditorView({ hasFocus: false })));
    const report = await timeOut(page);

    // Clicked at t=1000; the view was there at the first editor poll, t=2500;
    // the deadline check ran at t=16000.
    assert.ok(
      report.result.diagnosis.includes(
        "timing: 15000 ms after new_clicked (deadline 15000 ms); " +
          "editor view first seen 1500 ms after new_clicked",
      ),
      JSON.stringify(report.result.diagnosis),
    );
  },
);

test(
  "editor timeout: a view that never appeared says never",
  { concurrency: false },
  async () => {
    const report = await timeOut(pageWith((p) => (p.view = undefined)));
    assert.ok(
      report.result.diagnosis.includes(
        "timing: 15000 ms after new_clicked (deadline 15000 ms); editor view first seen never",
      ),
      JSON.stringify(report.result.diagnosis),
    );
  },
);

test(
  "a failure that is not the editor timeout carries no diagnosis",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setElement(
      '[data-source-focus-launch="start-new"]',
      new FakeElement({ dispatchResult: false }),
    );

    const report = await exchange("launch", 1);

    assert.equal(report.result.ok, false);
    assert.equal("diagnosis" in report.result, false);
  },
);

test(
  "a ready editor still passes the phase: the diagnosis changes nothing on the happy path",
  { concurrency: false },
  async () => {
    const page = pageWith();
    page.dom.setElement('[data-source-focus-launch="mode-preview"]', new FakeElement());
    await exchange("launch", 1);
    install(page);

    const report = await exchange("editor", 2, { exchangeId: 1, phase: "launch" });

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "preview_clicked");
  },
);
