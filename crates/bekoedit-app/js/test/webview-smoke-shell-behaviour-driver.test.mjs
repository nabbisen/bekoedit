// Phase coverage for shell_behaviour_driver.js -- the second WebView run
// (RFC-044 slice-1 §4/§5), against the simulated tree fixture in
// webview-smoke-tree-fake.mjs. Same method as
// webview-smoke-driver-phases.test.mjs: no real WebView, no jsdom, no
// display -- a controllable fake standing in for exactly what the driver
// reads or calls.
//
// Row indices below match the real tree: row 0 is always the workspace
// root itself (auto-expanded on mount, per collect_rows pushing the root
// before its children -- dioxus-swdir-tree-core's tree.rs), rows 1-4 are
// the four seeded entries (sub, a.md, notes.txt, z.md). Missing the root
// row was a real finding from CI against a real WebView (RFC-044 slice-1);
// FakeTree models it explicitly now so this suite would have caught it.
//
// Every assertion here was proven able to fail before being trusted: see
// the review request's mutation table (a scratch copy of
// shell_behaviour_driver.js, mutated one line at a time, this suite
// re-run against it).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { FakeTree } from "./webview-smoke-tree-fake.mjs";
import {
  FakeDioxus,
  acknowledgement,
  request,
  runDriver as runDriverWith,
} from "./webview-smoke-driver-harness.mjs";

const driverSource = readFileSync(
  new URL("../../src/webview_smoke/shell_behaviour_driver.js", import.meta.url),
  "utf8",
);

function runDriver(dioxus) {
  return runDriverWith(driverSource, dioxus);
}

/** Runs one full request -> report -> acknowledgement -> completion cycle
 * and returns the report, exactly as one `document::eval` round-trip
 * would. State persists across calls via
 * `window.__bkWebViewShellBehaviourState`. */
async function exchange(phase, exchangeId, release = null) {
  const dioxus = new FakeDioxus();
  const completion = runDriver(dioxus);
  dioxus.push(request(exchangeId, phase, release));
  const report = await dioxus.nextSent();
  dioxus.push(acknowledgement(report));
  await completion;
  return report;
}

/** expand_enter is multi-call and pollable (move onto sub, dispatch
 * expand, confirm expand + dispatch enter, confirm enter) because
 * expanding an unscanned directory is real async work (RFC-044 slice-1,
 * expand-timeout fix). Against the fake tree, whose mutations are
 * synchronous, that still takes exactly four calls -- each
 * `document::eval` round-trip only checks and acts once. Returns the
 * final report plus the `{exchangeId, phase}` to release from the next
 * phase. */
async function driveExpandEnter(startExchangeId, release) {
  await exchange("expand_enter", startExchangeId, release);
  await exchange("expand_enter", startExchangeId + 1, {
    exchangeId: startExchangeId,
    phase: "expand_enter",
  });
  await exchange("expand_enter", startExchangeId + 2, {
    exchangeId: startExchangeId + 1,
    phase: "expand_enter",
  });
  const report = await exchange("expand_enter", startExchangeId + 3, {
    exchangeId: startExchangeId + 2,
    phase: "expand_enter",
  });
  return {
    report,
    nextExchangeId: startExchangeId + 4,
    release: { exchangeId: startExchangeId + 3, phase: "expand_enter" },
  };
}

test(
  "down_up: focuses the first row (the workspace root) directly, then Down/Down/Up/Up moves and the tab-stop invariant holds throughout",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();

    const report = await exchange("down_up", 1);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "down_up_moved");
    assert.equal(tree.activeIndex, 0, "Down,Down,Up,Up from row 0 must return to row 0");
  },
);

test(
  "down_up: a broken invariant (two rows at tabindex=0) is a terminal failure naming it",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const realHandleKey = tree.handleKey.bind(tree);
    tree.handleKey = (index, key) => {
      realHandleKey(index, key);
      // Break the invariant right after the first Down: pretend row 0 is
      // still also a tabindex=0 row by never letting activeIndex move off
      // it for the *reported* attribute -- simulate via a second active row.
      if (key === "ArrowDown" && tree.activeIndex === 1) {
        tree.brokenInvariant = true;
      }
    };
    const originalElements = tree.elements.bind(tree);
    tree.elements = () => {
      const elements = originalElements();
      if (tree.brokenInvariant && elements[0]) {
        const original = elements[0].getAttribute.bind(elements[0]);
        elements[0].getAttribute = (name) => (name === "tabindex" ? "0" : original(name));
      }
      return elements;
    };

    const report = await exchange("down_up", 1);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /roving-tabindex invariant/);
  },
);

test(
  "expand_enter: moves onto sub, dispatch is pending, expand confirmation is pending, entering the child is progress",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    await exchange("down_up", 1);

    const moved = await exchange("expand_enter", 2, { exchangeId: 1, phase: "down_up" });
    assert.equal(moved.kind, "pending");
    assert.equal(tree.activeIndex, 1, "the first call must move focus onto sub (row 1)");

    const dispatched = await exchange("expand_enter", 3, { exchangeId: 2, phase: "expand_enter" });
    assert.equal(dispatched.kind, "pending");

    const expandConfirmed = await exchange("expand_enter", 4, {
      exchangeId: 3,
      phase: "expand_enter",
    });
    assert.equal(expandConfirmed.kind, "pending");
    assert.equal(
      tree.root.children[0].isExpanded,
      true,
      "expand must already be confirmed by the third call",
    );

    const report = await exchange("expand_enter", 5, { exchangeId: 4, phase: "expand_enter" });

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "expand_entered");
    assert.equal(tree.activeIndex, 2, "the second Right must move focus into the child row");
  },
);

test(
  "expand_enter: a directory that never expands times out naming the phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    tree.setTime(0);
    // Sabotage: ArrowRight on a directory normally sets isExpanded; freeze it.
    const realHandleKey = tree.handleKey.bind(tree);
    tree.handleKey = (index, key) => {
      if (key === "ArrowRight" && tree.visibleRows()[index]?.node.isDir) return;
      realHandleKey(index, key);
    };
    await exchange("down_up", 1);
    await exchange("expand_enter", 2, { exchangeId: 1, phase: "down_up" }); // moves onto sub

    // Second call: dispatches ArrowRight (a no-op here), sets the deadline, pending.
    const dispatched = await exchange("expand_enter", 3, { exchangeId: 2, phase: "expand_enter" });
    assert.equal(dispatched.kind, "pending");
    const { deadline } = window.__bkWebViewShellBehaviourState;

    tree.setTime(deadline); // timedOut() uses >=, so exactly the deadline counts
    const report = await exchange("expand_enter", 4, { exchangeId: 3, phase: "expand_enter" });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at expand_enter/);
  },
);

test(
  "collapse_ascend: Left ascends from the child, Left again collapses the parent",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    await exchange("down_up", 1);
    const { nextExchangeId, release } = await driveExpandEnter(2, {
      exchangeId: 1,
      phase: "down_up",
    });

    const report = await exchange("collapse_ascend", nextExchangeId, release);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "collapse_ascended");
    assert.equal(tree.root.children[0].isExpanded, false);
    assert.equal(tree.activeIndex, 1);
  },
);

test(
  "home_end: End reaches the last row, Home reaches the first",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    await exchange("down_up", 1);
    const expandEnter = await driveExpandEnter(2, { exchangeId: 1, phase: "down_up" });
    await exchange("collapse_ascend", expandEnter.nextExchangeId, expandEnter.release);

    const report = await exchange("home_end", expandEnter.nextExchangeId + 1, {
      exchangeId: expandEnter.nextExchangeId,
      phase: "collapse_ascend",
    });

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "home_end_reached");
    assert.equal(tree.activeIndex, 0, "must end back on the root row");
  },
);

test(
  "non_openable: Down reaches the disabled row without skipping it; Enter on it is a no-op, and this is the slice's terminal contract",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    await exchange("down_up", 1);
    const expandEnter = await driveExpandEnter(2, { exchangeId: 1, phase: "down_up" });
    await exchange("collapse_ascend", expandEnter.nextExchangeId, expandEnter.release);
    await exchange("home_end", expandEnter.nextExchangeId + 1, {
      exchangeId: expandEnter.nextExchangeId,
      phase: "collapse_ascend",
    });

    const report = await exchange("non_openable", expandEnter.nextExchangeId + 2, {
      exchangeId: expandEnter.nextExchangeId + 1,
      phase: "home_end",
    });

    // Contract 7 (Enter opens a document and the editor takes focus) is
    // deferred to task 014 -- OpenDocument does not claim editor focus
    // today -- so this slice's run terminates here, at contract 6.
    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(tree.activeIndex, 3, "must land on notes.txt, the fourth row");
    assert.equal(tree.openedPath, null, "Enter on a non-openable row must not open anything");
    assert.deepEqual(report.result.milestones, [
      "down_up_moved",
      "expand_entered",
      "collapse_ascended",
      "home_end_reached",
      "non_openable_reachable",
    ]);
  },
);

test(
  "non_openable: a row that opens anyway on Enter is a terminal failure naming it",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    // Sabotage: make the non-openable row activate like an openable file.
    tree.root.children[2].isOpenable = true;
    await exchange("down_up", 1);
    const expandEnter = await driveExpandEnter(2, { exchangeId: 1, phase: "down_up" });
    await exchange("collapse_ascend", expandEnter.nextExchangeId, expandEnter.release);
    await exchange("home_end", expandEnter.nextExchangeId + 1, {
      exchangeId: expandEnter.nextExchangeId,
      phase: "collapse_ascend",
    });

    // aria-disabled is driven by isDir||isOpenable, so making it openable
    // also makes it report non-disabled -- the driver must catch *that*,
    // not silently proceed to open it.
    const report = await exchange("non_openable", expandEnter.nextExchangeId + 2, {
      exchangeId: expandEnter.nextExchangeId + 1,
      phase: "home_end",
    });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /not actually non-openable/);
  },
);
