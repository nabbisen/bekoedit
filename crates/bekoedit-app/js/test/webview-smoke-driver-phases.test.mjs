// Phase/DOM coverage for driver.js (task 012).
//
// webview-smoke-driver.test.mjs covers the RFC-041 evaluator-pin
// protocol thoroughly. It stubs document.querySelector to return null
// unconditionally, so every run there takes the launch phase's
// "element not found -> pending" branch -- no phase transition,
// readiness check, timeout, or error-toast path is ever reached. This
// file drives those five paths with the shared, controllable DOM fake
// (webview-smoke-dom-fake.mjs) instead.
//
// Each test here was proven able to fail before being trusted: see the
// review request's §2/§5 mutation tables for the method (a scratch copy
// of driver.js, mutated one line at a time, with this suite re-run
// against it), since driver.js is evaluated from a string and no
// coverage tool can see it.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  FakeDom,
  FakeElement,
  FakeEditorView,
  errorToastElement,
} from "./webview-smoke-dom-fake.mjs";
import {
  FakeDioxus,
  acknowledgement,
  request,
  runDriver as runDriverWith,
} from "./webview-smoke-driver-harness.mjs";

const driverSource = readFileSync(
  new URL("../../src/webview_smoke/driver.js", import.meta.url),
  "utf8",
);
const MARKER = "RFC041_WEBVIEW_SMOKE_MARKER";

function runDriver(dioxus) {
  return runDriverWith(driverSource, dioxus);
}

/** Runs one full request -> report -> acknowledgement -> completion
 * cycle, exactly as one `document::eval` round-trip would, and returns
 * the report. `driver.js`'s state persists across calls via
 * `window.__bkWebViewSmokeState`, so a staged sequence of phases is
 * built by calling this repeatedly with the previous exchange's
 * (exchangeId, phase) as `release`. */
async function exchange(phase, exchangeId, release = null) {
  const dioxus = new FakeDioxus();
  const completion = runDriver(dioxus);
  dioxus.push(request(exchangeId, phase, release));
  const report = await dioxus.nextSent();
  dioxus.push(acknowledgement(report));
  await completion;
  return report;
}

test(
  "launch: New button exists -> clicked, milestone recorded, deadline set, phase advances to editor",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(1000);
    const newButton = new FakeElement();
    dom.setElement('[data-source-focus-launch="start-new"]', newButton);

    const report = await exchange("launch", 1);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "new_clicked");
    assert.equal(newButton.dispatchedEvents.length, 1);
    assert.equal(newButton.dispatchedEvents[0].type, "click");

    const state = window.__bkWebViewSmokeState;
    assert.equal(state.phase, "editor", "phase must advance to editor");
    assert.equal(state.deadline, 1000 + 15000, "deadline must be set from performance.now()");
    assert.ok(state.milestones.includes("new_clicked"));
  },
);

test(
  "launch: a refused New click is a terminal failure naming it, not progress",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    const newButton = new FakeElement({ dispatchResult: false });
    dom.setElement('[data-source-focus-launch="start-new"]', newButton);

    const report = await exchange("launch", 1);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /New click was not accepted/);
  },
);

test(
  "editor: not-ready is pending; ready dispatches the edit, clicks Preview, and advances to preview",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    dom.setElement('[data-source-focus-launch="start-new"]', new FakeElement());
    const launchReport = await exchange("launch", 1);
    assert.equal(launchReport.kind, "progress");

    // Not ready yet: no editor view has mounted.
    const pendingReport = await exchange("editor", 2, { exchangeId: 1, phase: "launch" });
    assert.equal(pendingReport.kind, "pending");

    // Ready: connected, focused view; host carries no status marker;
    // FakeElement's default querySelector already returns null for that.
    const view = new FakeEditorView({ connected: true, hasFocus: true, docLength: 7 });
    dom.setEditorView(view);
    dom.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
    const previewButton = new FakeElement();
    dom.setElement('[data-source-focus-launch="mode-preview"]', previewButton);

    const readyReport = await exchange("editor", 3, { exchangeId: 2, phase: "editor" });

    assert.equal(readyReport.kind, "progress");
    assert.equal(readyReport.milestone, "preview_clicked");
    assert.deepEqual(view.dispatchCalls, [{ changes: { from: 7, insert: MARKER } }]);
    assert.equal(previewButton.dispatchedEvents.length, 1);
    assert.equal(window.__bkWebViewSmokeState.phase, "preview");
  },
);

test(
  "preview: marker absent is pending; marker present with the tab active is terminal success",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    dom.setElement('[data-source-focus-launch="start-new"]', new FakeElement());
    await exchange("launch", 1);

    dom.setEditorView(new FakeEditorView({ docLength: 0 }));
    dom.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
    dom.setElement('[data-source-focus-launch="mode-preview"]', new FakeElement());
    await exchange("editor", 2, { exchangeId: 1, phase: "launch" });

    // Tab already active, marker not written to the DOM yet -- the tab
    // being active on its own must not be enough. No article element
    // configured, so this also isolates the marker check from the
    // active-tab check: dropping the marker check while leaving `active`
    // truthy would incorrectly report success here.
    dom.setElement(
      '[data-source-focus-launch="mode-preview"].active[aria-selected="true"]',
      new FakeElement(),
    );
    const pendingPreview = await exchange("preview", 3, { exchangeId: 2, phase: "editor" });
    assert.equal(pendingPreview.kind, "pending");

    // Marker present too now.
    dom.setElement(
      "article.preview",
      new FakeElement({ textContent: `before ${MARKER} after` }),
    );
    const finalReport = await exchange("preview", 4, { exchangeId: 3, phase: "preview" });

    assert.equal(finalReport.kind, "terminal");
    assert.equal(finalReport.result.ok, true);
    assert.equal(finalReport.result.stage, "preview_verified");
  },
);

test(
  "editor: an elapsed deadline fails, naming the stage it timed out at",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(1000);
    dom.setElement('[data-source-focus-launch="start-new"]', new FakeElement());
    await exchange("launch", 1); // deadline = 1000 + 15000 = 16000

    dom.setTime(16000); // timedOut() uses >=, so exactly the deadline counts
    const timedOutReport = await exchange("editor", 2, { exchangeId: 1, phase: "launch" });

    assert.equal(timedOutReport.kind, "terminal");
    assert.equal(timedOutReport.result.ok, false);
    assert.match(timedOutReport.result.error, /timed out at editor_ready_focused/);
  },
);

test(
  "an error toast observed via mutation fails the run even after every milestone is reached",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    dom.setElement('[data-source-focus-launch="start-new"]', new FakeElement());
    await exchange("launch", 1);

    dom.setEditorView(new FakeEditorView({ docLength: 0 }));
    dom.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
    dom.setElement('[data-source-focus-launch="mode-preview"]', new FakeElement());
    await exchange("editor", 2, { exchangeId: 1, phase: "launch" });

    // The toast appears mid-run, observed through driver.js's own
    // MutationObserver -- not the initial document.querySelector check
    // createState() also makes.
    dom.emitMutation({ addedNodes: [errorToastElement()] });

    dom.setElement("article.preview", new FakeElement({ textContent: MARKER }));
    dom.setElement(
      '[data-source-focus-launch="mode-preview"].active[aria-selected="true"]',
      new FakeElement(),
    );
    const finalReport = await exchange("preview", 3, { exchangeId: 2, phase: "editor" });

    assert.equal(finalReport.kind, "terminal");
    assert.equal(finalReport.result.ok, false);
    assert.match(finalReport.result.error, /an error toast appeared/);
    assert.ok(
      finalReport.result.milestones.includes("preview_clicked"),
      "every milestone reached before the toast was observed must still be reported",
    );
  },
);
