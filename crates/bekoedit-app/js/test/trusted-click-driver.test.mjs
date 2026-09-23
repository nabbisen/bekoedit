// Phase/DOM coverage for trusted_click_driver.js (task 023).
//
// The Rust-side tests (webview_smoke/trusted_click/tests.rs) cover the
// phase machine's own transition/validation logic; this file is what task
// 023 §5.4 actually asked for and did not originally deliver (2026-09-23
// review §2) -- unit coverage of the driver script itself, on the
// project's existing shared-fake suite (webview-smoke-driver-harness.mjs,
// webview-smoke-dom-fake.mjs), the same pattern driver.js and
// shell_behaviour_driver.js already use.
//
// Every test here was checked against a mutated scratch copy of the
// driver at least once while writing this file (flip the activeElement
// comparison, drop the errorToastSeen check, change a deadline
// arithmetic sign) and confirmed to fail -- see the review request for
// which.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { FakeDom, FakeElement, FakeEditorView, errorToastElement } from "./webview-smoke-dom-fake.mjs";
import {
  FakeDioxus,
  acknowledgement,
  request,
  runDriver as runDriverWith,
} from "./webview-smoke-driver-harness.mjs";

const driverSource = readFileSync(
  new URL("../../src/webview_smoke/trusted_click_driver.js", import.meta.url),
  "utf8",
);
const MARKER = "TASK023_TRUSTED_CLICK_MARKER";

function runDriver(dioxus) {
  return runDriverWith(driverSource, dioxus);
}

/** Same shape as webview-smoke-driver-phases.test.mjs's own `exchange`:
 * one full request -> report -> acknowledgement -> completion cycle. */
async function exchange(phase, exchangeId, release = null) {
  const dioxus = new FakeDioxus();
  const completion = runDriver(dioxus);
  dioxus.push(request(exchangeId, phase, release));
  const report = await dioxus.nextSent();
  dioxus.push(acknowledgement(report));
  await completion;
  return report;
}

/** Sets up the DOM so `editorFocused(fileName)` reads true: a connected,
 * focused view, a status-free host, and `.file-name` matching. */
function focusEditorOn(dom, fileName) {
  dom.setEditorView(new FakeEditorView({ connected: true, hasFocus: true }));
  dom.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
  dom.setElement(".file-name", new FakeElement({ textContent: fileName }));
}

test(
  "proof_of_trust: activeElement elsewhere is pending; becoming the trigger reports progress and advances",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    const trigger = new FakeElement();
    dom.setElementById("app-menu-trigger", trigger);

    const pending = await exchange("proof_of_trust", 1);
    assert.equal(pending.kind, "pending");

    dom.setActiveElement(trigger);
    const report = await exchange("proof_of_trust", 2, { exchangeId: 1, phase: "proof_of_trust" });

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "trusted_click_focused_default_target");
    assert.equal(window.__bkTrustedClickState.phase, "tree_row_focus");
  },
);

test(
  "proof_of_trust: an elapsed deadline fails, naming the stage it timed out at",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    dom.setElementById("app-menu-trigger", new FakeElement());

    await exchange("proof_of_trust", 1); // deadline = 0 + 10000

    dom.setTime(10000); // timedOut() uses >=, so exactly the deadline counts
    const report = await exchange("proof_of_trust", 2, { exchangeId: 1, phase: "proof_of_trust" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at trusted_click_focused_default_target/);
  },
);

test(
  "tree_row_focus: editor not focused on child.md is pending; focused reports progress and advances",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    const trigger = new FakeElement();
    dom.setElementById("app-menu-trigger", trigger);
    dom.setActiveElement(trigger);
    await exchange("proof_of_trust", 1);

    const pending = await exchange("tree_row_focus", 2, { exchangeId: 1, phase: "proof_of_trust" });
    assert.equal(pending.kind, "pending");

    focusEditorOn(dom, "child.md");
    const report = await exchange("tree_row_focus", 3, { exchangeId: 2, phase: "tree_row_focus" });

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "tree_row_trusted_click_focused_editor");
    assert.equal(window.__bkTrustedClickState.phase, "backlink_focus");
  },
);

test(
  "tree_row_focus: focused on the wrong file is still pending",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    const trigger = new FakeElement();
    dom.setElementById("app-menu-trigger", trigger);
    dom.setActiveElement(trigger);
    await exchange("proof_of_trust", 1);

    // Focused, but on parent.md rather than child.md -- must not be
    // mistaken for tree_row_focus's own target.
    focusEditorOn(dom, "parent.md");
    const report = await exchange("tree_row_focus", 2, { exchangeId: 1, phase: "proof_of_trust" });

    assert.equal(report.kind, "pending");
  },
);

test(
  "tree_row_focus: an elapsed deadline fails, naming the stage it timed out at",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    const trigger = new FakeElement();
    dom.setElementById("app-menu-trigger", trigger);
    dom.setActiveElement(trigger);
    await exchange("proof_of_trust", 1); // advances to tree_row_focus, deadline = 0 + 10000

    dom.setTime(10000);
    const report = await exchange("tree_row_focus", 2, { exchangeId: 1, phase: "proof_of_trust" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at tree_row_trusted_click_focused_editor/);
  },
);

async function reachBacklinkFocus(dom) {
  const trigger = new FakeElement();
  dom.setElementById("app-menu-trigger", trigger);
  dom.setActiveElement(trigger);
  await exchange("proof_of_trust", 1);
  focusEditorOn(dom, "child.md");
  await exchange("tree_row_focus", 2, { exchangeId: 1, phase: "proof_of_trust" });
}

test(
  "backlink_focus: editor not focused on parent.md is pending; focused is terminal success",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachBacklinkFocus(dom);

    const pending = await exchange("backlink_focus", 3, { exchangeId: 2, phase: "tree_row_focus" });
    assert.equal(pending.kind, "pending");

    focusEditorOn(dom, "parent.md");
    const report = await exchange("backlink_focus", 4, { exchangeId: 3, phase: "backlink_focus" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "backlink_trusted_click_focused_editor");
    assert.equal(report.result.marker, MARKER);
    assert.deepEqual(report.result.milestones, [
      "trusted_click_focused_default_target",
      "tree_row_trusted_click_focused_editor",
      "backlink_trusted_click_focused_editor",
    ]);
  },
);

test(
  "backlink_focus: an elapsed deadline fails, naming the stage it timed out at",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachBacklinkFocus(dom); // advances to backlink_focus, deadline = 0 + 10000

    dom.setTime(10000);
    const report = await exchange("backlink_focus", 3, { exchangeId: 2, phase: "tree_row_focus" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at backlink_trusted_click_focused_editor/);
  },
);

test(
  "backlink_focus: an error toast observed via mutation fails the run even after the editor is focused",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachBacklinkFocus(dom);

    // The toast appears mid-run, observed through the driver's own
    // MutationObserver -- not the initial document.querySelector check
    // createState() also makes.
    dom.emitMutation({ addedNodes: [errorToastElement()] });

    focusEditorOn(dom, "parent.md");
    const report = await exchange("backlink_focus", 3, { exchangeId: 2, phase: "tree_row_focus" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /an error toast appeared/);
    assert.ok(
      report.result.milestones.includes("tree_row_trusted_click_focused_editor"),
      "every milestone reached before the toast was observed must still be reported",
    );
  },
);

test(
  "an out-of-order phase request is a phase-mismatch terminal failure",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    dom.setElementById("app-menu-trigger", new FakeElement());

    // Requests backlink_focus while the driver's own state is still at
    // proof_of_trust -- the state.phase check, not the transport's own
    // ordering (which is Rust-side and not exercised by this harness).
    const report = await exchange("backlink_focus", 1);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /phase mismatch: requested backlink_focus, current proof_of_trust/);
  },
);
