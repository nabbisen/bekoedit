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
  "backlink_focus: editor not focused on parent.md is pending; focused reports progress and advances",
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

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "backlink_trusted_click_focused_editor");
    assert.equal(window.__bkTrustedClickState.phase, "mode_tab_focus");
  },
);

async function reachModeTabFocus(dom) {
  await reachBacklinkFocus(dom);
  focusEditorOn(dom, "parent.md");
  await exchange("backlink_focus", 3, { exchangeId: 2, phase: "tree_row_focus" });
}

test(
  "mode_tab_focus: the Text tab not yet active is pending; active with the editor focused is terminal success",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachModeTabFocus(dom);

    const pending = await exchange("mode_tab_focus", 4, { exchangeId: 3, phase: "backlink_focus" });
    assert.equal(pending.kind, "pending");

    dom.setElement(
      '[data-source-focus-launch="mode-text"].active[aria-selected="true"]',
      new FakeElement(),
    );
    const report = await exchange("mode_tab_focus", 5, { exchangeId: 4, phase: "mode_tab_focus" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "mode_tab_trusted_click_focused_editor");
    assert.equal(report.result.marker, MARKER);
    assert.deepEqual(report.result.milestones, [
      "trusted_click_focused_default_target",
      "tree_row_trusted_click_focused_editor",
      "backlink_trusted_click_focused_editor",
      "mode_tab_trusted_click_focused_editor",
    ]);
  },
);

test(
  "mode_tab_focus: the tab active but the editor not yet focused is still pending",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachModeTabFocus(dom);

    // reachModeTabFocus already left the editor focused (needed to reach
    // backlink_focus's own success) -- un-focus it again so this test
    // isolates the tab check from the editorFocused check, same shape as
    // driver-phases.test.mjs's own preview marker/active-tab isolation
    // test. Active tab alone must not be enough.
    dom.setEditorView(new FakeEditorView({ connected: true, hasFocus: false }));
    dom.setElement(
      '[data-source-focus-launch="mode-text"].active[aria-selected="true"]',
      new FakeElement(),
    );
    const report = await exchange("mode_tab_focus", 4, { exchangeId: 3, phase: "backlink_focus" });

    assert.equal(report.kind, "pending");
  },
);

test(
  "mode_tab_focus: an elapsed deadline fails, naming the stage it timed out at",
  { concurrency: false },
  async () => {
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    await reachModeTabFocus(dom); // advances to mode_tab_focus, deadline = 0 + 10000

    dom.setTime(10000);
    const report = await exchange("mode_tab_focus", 4, { exchangeId: 3, phase: "backlink_focus" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at mode_tab_trusted_click_focused_editor/);
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

test(
  "an unrecognized phase name sends a named diagnostic instead of hanging silently",
  { concurrency: false },
  async () => {
    // 2026-09-23 finding review §4.3: this is exactly the shape every
    // real hang across sixteen CI runs had -- a phase name the driver's
    // own `phases` array does not recognize. Before failEarly existed,
    // this threw before any dioxus.send() at all, so nothing reached
    // Rust's own eval.recv() and it waited out the full 5 s cap in
    // silence. Constructs the malformed request directly (request()
    // always builds a well-formed one) and drives the raw channel, since
    // this path rejects before exchange()'s own report/ack cycle can run.
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);

    const dioxus = new FakeDioxus();
    const completion = runDriver(dioxus);
    dioxus.push({
      protocolVersion: 2,
      exchangeId: 1,
      phase: "a_phase_the_driver_does_not_know",
      releaseExchangeId: null,
      releasePhase: null,
    });

    const sent = await dioxus.nextSent();
    assert.equal(sent.kind, "terminal");
    assert.equal(sent.result.ok, false);
    assert.match(sent.result.error, /invalid phase request/);
    assert.match(sent.result.error, /a_phase_the_driver_does_not_know/);

    // The driver's own promise rejects right after sending -- expected,
    // and not what this test is about (a report was sent at all, fast).
    await assert.rejects(completion);
  },
);

test(
  "a bad acknowledgement and an occupied pin return their reasons in the completion",
  { concurrency: false },
  async () => {
    // Task 025 review §3.1: returned, not thrown -- a thrown message does not
    // survive eval.join() on WebKitGTK.
    const dom = new FakeDom();
    dom.install();
    dom.setTime(0);
    const bad = new FakeDioxus();
    const badRun = runDriver(bad);
    bad.push(request(1, "proof_of_trust"));
    const badReport = await bad.nextSent();
    bad.push({ ...acknowledgement(badReport), exchangeId: 2 });
    const badReturned = await badRun;
    assert.equal(badReturned.error, "invalid phase acknowledgement");
    assert.equal(badReturned.acknowledgementProcessed, false);
    assert.equal(badReturned.evaluatorPinned, false);
    assert.equal(window.__bkTrustedClickEvalPin.current, null);

    // The failed exchange left no pin, so the next one starts clean; another
    // pin appears between its pre-checks and its footer.
    const occupied = new FakeDioxus();
    const occupiedRun = runDriver(occupied);
    occupied.push(request(3, "proof_of_trust"));
    const occupiedReport = await occupied.nextSent();
    const occupant = { occupied: true };
    window.__bkTrustedClickEvalPin.current = occupant;
    occupied.push(acknowledgement(occupiedReport));
    const occupiedReturned = await occupiedRun;
    assert.equal(occupiedReturned.error, "trusted-click evaluator pin was already occupied");
    assert.equal(occupiedReturned.evaluatorPinned, false);
    assert.equal(window.__bkTrustedClickEvalPin.current, occupant);
  },
);

// ---- Task 029: a timeout says what the page looked like ------------------
//
// The `tree_row_focus` timeout was seen three times with the same one-line
// message. These tests stage each case the fakes can (a: the click never
// landed; b: it reached the row but no document opened; c: the document
// opened but the editor never became ready; d: the editor is ready and focus
// was refused) and one mid-wait change (how e, focus lost after it landed,
// would surface), and assert the message tells them apart. Case e itself
// cannot be staged: any poll that sees focus in the editor on child.md
// reports success, so a timeout can only show it as a *change*.

const TREE_ROWS = ".tree-row.tree-file";
const ROW_CLASS = "tree-row tree-file";

function treeRow(name, selected, { active = false } = {}) {
  const row = new FakeElement({ tagName: "BUTTON", className: ROW_CLASS, textContent: name })
    .withAttribute("aria-selected", String(selected));
  row.isActiveRow = active;
  return row;
}

/** Drives proof_of_trust to completion, then polls tree_row_focus once at
 * `firstPollAt`; returns the `next(atMs)` that polls again at that time. */
async function pollTreeRow(dom, firstPollAt = 100) {
  const trigger = new FakeElement({ tagName: "BUTTON", id: "app-menu-trigger" });
  dom.setElementById("app-menu-trigger", trigger);
  dom.setActiveElement(trigger);
  dom.setTime(0);
  await exchange("proof_of_trust", 1);
  dom.setTime(firstPollAt);
  let exchangeId = 2;
  let release = { exchangeId: 1, phase: "proof_of_trust" };
  const next = async (atMs) => {
    dom.setTime(atMs);
    const report = await exchange("tree_row_focus", exchangeId, release);
    release = { exchangeId, phase: "tree_row_focus" };
    exchangeId += 1;
    return report;
  };
  await next(firstPollAt);
  return { trigger, next };
}

async function timeoutMessageAfter(dom, next) {
  const report = await next(10000);
  assert.equal(report.kind, "terminal");
  assert.match(report.result.error, /^Error: timed out at tree_row_trusted_click_focused_editor\./);
  return report.result.error;
}

test("timeout, case a: the click never landed (focus still on the menu trigger, no file, no editor)", { concurrency: false }, async () => {
  const dom = new FakeDom();
  dom.install();
  dom.setElements(TREE_ROWS, [treeRow("child.md", false)]);
  const { next } = await pollTreeRow(dom);
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /activeElement=button#app-menu-trigger inEditorHost=false/);
  assert.match(message, /openFile=null treeRows=\[child\.md\(selected=false\)\]/);
  assert.match(message, /editor: view=false/);
  assert.match(message, /no change in the whole wait/);
});

test("timeout, case b: the click reached the row (it holds focus) but no document opened", { concurrency: false }, async () => {
  const dom = new FakeDom();
  dom.install();
  const row = treeRow("child.md", false);
  row.className = ROW_CLASS;
  dom.setElements(TREE_ROWS, [row]);
  const { next } = await pollTreeRow(dom);
  dom.setActiveElement(row);
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /activeElement=button\.tree-row\.tree-file inEditorHost=false/);
  assert.match(message, /openFile=null treeRows=\[child\.md\(selected=false\)\]/);
});

test("timeout, case c: the document opened (row selected, file named) but the editor never became ready", { concurrency: false }, async () => {
  const dom = new FakeDom();
  dom.install();
  dom.setElements(TREE_ROWS, [treeRow("child.md", true)]);
  dom.setElement(".file-name", new FakeElement({ textContent: "child.md" }));
  dom.setElement(
    '[data-source-focus-launch-region="text"]',
    new FakeElement().withChild(".source-editor-status", new FakeElement({ textContent: "Loading editor" })),
  );
  const { next } = await pollTreeRow(dom);
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /openFile=child\.md treeRows=\[child\.md\(selected=true\)\]/);
  assert.match(message, /editor: view=false connected=undefined hasFocus=undefined host=true statusMarker=Loading editor/);
});

test("timeout, case d: the editor is ready on child.md but focus was refused (it stays on the tree row)", { concurrency: false }, async () => {
  const dom = new FakeDom();
  dom.install();
  const row = treeRow("child.md", true);
  dom.setElements(TREE_ROWS, [row]);
  dom.setElement(".file-name", new FakeElement({ textContent: "child.md" }));
  dom.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
  dom.setEditorView(new FakeEditorView({ connected: true, hasFocus: false }));
  const { next } = await pollTreeRow(dom);
  dom.setActiveElement(row);
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /activeElement=button\.tree-row\.tree-file inEditorHost=false/);
  assert.match(message, /openFile=child\.md treeRows=\[child\.md\(selected=true\)\]/);
  assert.match(message, /editor: view=true connected=true hasFocus=false host=true statusMarker=null/);
});

test("timeout: the document not having focus is reported, so a backgrounded window is not mistaken for a refused claim", { concurrency: false }, async () => {
  const dom = new FakeDom();
  dom.install();
  dom.setElements(TREE_ROWS, [treeRow("child.md", false)]);
  const { next } = await pollTreeRow(dom);
  dom.setDocumentHasFocus(false);
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /At the first poll after the click: document\.hasFocus\(\)=true;/);
  assert.match(message, /At the timeout: document\.hasFocus\(\)=false;/);
});

test("timeout: a change mid-wait is reported with its time, and shows both where the wait started and ended", { concurrency: false }, async () => {
  // How case e would surface: what the page looked like at the first poll,
  // the first moment it differed, and how long after.
  const dom = new FakeDom();
  dom.install();
  const row = treeRow("child.md", false);
  dom.setElements(TREE_ROWS, [row]);
  const { next } = await pollTreeRow(dom, 100);
  row.withAttribute("aria-selected", "true");
  dom.setElement(".file-name", new FakeElement({ textContent: "child.md" }));
  const pending = await next(400);
  assert.equal(pending.kind, "pending");
  const message = await timeoutMessageAfter(dom, next);
  assert.match(message, /At the first poll after the click: (?:(?!At the).)*?openFile=null treeRows=\[child\.md\(selected=false\)\]/);
  assert.match(message, /At the timeout: (?:(?!At the).)*?openFile=child\.md treeRows=\[child\.md\(selected=true\)\]/);
  assert.match(message, /first change 300 ms after the first poll, to: (?:(?!At the).)*?openFile=child\.md/);
});
