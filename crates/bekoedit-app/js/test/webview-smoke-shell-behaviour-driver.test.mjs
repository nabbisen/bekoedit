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
import { FakeElement, FakeEditorView } from "./webview-smoke-dom-fake.mjs";
import { FakeTree } from "./webview-smoke-tree-fake.mjs";
import { FakeMenu } from "./webview-smoke-menu-fake.mjs";
import { FakeModeTabs } from "./webview-smoke-tabs-fake.mjs";
import { FakeConflict, FakeRecovery, FakeSettings } from "./webview-smoke-screens-fake.mjs";
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
  // Slice 3 §4.1: the Recovery phases now run first. A test that starts at
  // down_up passes them against a default Recovery fake; stage 2's own
  // Recovery tests set `skipRecoveryPreamble` and drive them directly.
  const tree = globalThis.__bkFakeTree;
  if (
    phase === "down_up" &&
    release === null &&
    tree &&
    !tree.skipRecoveryPreamble &&
    window.__bkWebViewShellBehaviourState === undefined
  ) {
    new FakeRecovery(tree).install();
    const preamble = { exchangeId: 9001, release: null };
    await stepToResult("recovery_entry", preamble);
    await stepToResult("recovery_exit", preamble);
    release = preamble.release;
  }
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
  "non_openable: Down reaches the disabled row without skipping it; Enter on it is a no-op",
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

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "non_openable_reachable");
    assert.equal(tree.activeIndex, 3, "must land on notes.txt, the fourth row");
    assert.equal(tree.openedPath, null, "Enter on a non-openable row must not open anything");
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

/** Drives every phase up to and including non_openable, and returns the
 * exchange id and release to open enter_opens with. */
async function driveThroughNonOpenable() {
  await exchange("down_up", 1);
  const expandEnter = await driveExpandEnter(2, { exchangeId: 1, phase: "down_up" });
  await exchange("collapse_ascend", expandEnter.nextExchangeId, expandEnter.release);
  await exchange("home_end", expandEnter.nextExchangeId + 1, {
    exchangeId: expandEnter.nextExchangeId,
    phase: "collapse_ascend",
  });
  await exchange("non_openable", expandEnter.nextExchangeId + 2, {
    exchangeId: expandEnter.nextExchangeId + 1,
    phase: "home_end",
  });
  return {
    exchangeId: expandEnter.nextExchangeId + 3,
    release: { exchangeId: expandEnter.nextExchangeId + 2, phase: "non_openable" },
  };
}

test(
  "enter_opens: Enter opens the markdown row; pending until the editor is mounted and focused, then progress",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    tree.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
    const start = await driveThroughNonOpenable();

    // First call: ArrowUp onto a.md, Enter -- nothing mounted yet.
    const dispatched = await exchange("enter_opens", start.exchangeId, start.release);
    assert.equal(dispatched.kind, "pending");
    assert.equal(tree.activeIndex, 2, "must have moved back onto the markdown row");
    assert.equal(tree.openedPath, "a.md");

    // Mounted but unfocused -- the contract's whole point; still pending.
    tree.setEditorView(new FakeEditorView({ connected: true, hasFocus: false }));
    const unfocused = await exchange("enter_opens", start.exchangeId + 1, {
      exchangeId: start.exchangeId,
      phase: "enter_opens",
    });
    assert.equal(unfocused.kind, "pending");

    tree.setEditorView(new FakeEditorView({ connected: true, hasFocus: true }));
    const report = await exchange("enter_opens", start.exchangeId + 2, {
      exchangeId: start.exchangeId + 1,
      phase: "enter_opens",
    });

    // No longer terminal: task 016's contracts follow it.
    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "enter_opened_editor_focused");
    assert.equal(window.__bkWebViewShellBehaviourState.phase, "search_result_opens");
    assert.equal(window.__bkWebViewShellBehaviourState.deadline, null);
  },
);

test(
  "enter_opens: an editor that never takes focus times out naming the observed state",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    tree.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
    const start = await driveThroughNonOpenable();
    await exchange("enter_opens", start.exchangeId, start.release);
    tree.setEditorView(new FakeEditorView({ connected: true, hasFocus: false }));
    // The fake clock advances on requestAnimationFrame ticks, so read the
    // deadline the driver actually recorded rather than assuming one.
    const { deadline } = window.__bkWebViewShellBehaviourState;

    tree.setTime(deadline); // timedOut() uses >=, so exactly the deadline counts
    const report = await exchange("enter_opens", start.exchangeId + 1, {
      exchangeId: start.exchangeId,
      phase: "enter_opens",
    });

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at enter_opens: view=true .*hasFocus=false/);
  },
);

// ---- task 016: handoff-activation contracts ------------------------------

/** The transport's per-exchange evaluator timeout (transport.rs,
 * PHASE_EVALUATOR_TIMEOUT). No exchange may hold longer while it waits. */
const TRANSPORT_EVALUATOR_BUDGET_MS = 5000;

/** Steps a pollable phase until it stops returning pending. If it is still
 * pending after `expireAfter` exchanges, moves the fake clock to the phase's
 * own deadline, so its named timeout fires on the next exchange. */
async function stepToResult(phase, cursor, { expireAfter = 3 } = {}) {
  let report = await step(phase, cursor);
  let calls = 1;
  while (report.kind === "pending") {
    if (calls >= expireAfter) {
      globalThis.__bkFakeTree.setTime(window.__bkWebViewShellBehaviourState.deadline);
    }
    if (calls > expireAfter + 2) throw new Error(`${phase} never settled`);
    report = await step(phase, cursor);
    calls += 1;
  }
  return report;
}

/** One exchange of `phase` at `cursor`, then moves the cursor on. */
async function step(phase, cursor) {
  const report = await exchange(phase, cursor.exchangeId, cursor.release);
  cursor.release = { exchangeId: cursor.exchangeId, phase };
  cursor.exchangeId += 1;
  return report;
}

/** Drives every phase through enter_opens's progress. */
async function driveThroughEnterOpens(tree) {
  tree.setElement('[data-source-focus-launch-region="text"]', new FakeElement());
  const start = await driveThroughNonOpenable();
  const cursor = { exchangeId: start.exchangeId, release: start.release };
  await step("enter_opens", cursor);
  tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 4 }));
  const report = await step("enter_opens", cursor);
  assert.equal(report.kind, "progress");
  return cursor;
}

/** The search trigger, input and one `child.md` result, wired so clicking the
 * trigger opens the panel, as explorer.rs does. */
function installSearch(tree) {
  const trigger = new FakeElement();
  const input = new FakeElement();
  const result = new FakeElement({ textContent: "sub/child.md:1# child" });
  trigger.onClick = () => tree.setElement("#workspace-search-input", input);
  tree.setElement("#workspace-search-trigger", trigger);
  return { trigger, input, result };
}

/** Drives search_result_opens to its final, readiness-polling step. */
async function driveSearchToActivation(tree, cursor) {
  const search = installSearch(tree);
  tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 4 }));
  assert.equal((await step("search_result_opens", cursor)).kind, "pending");
  assert.equal(search.trigger.clicks, 1);
  assert.equal(search.input.value, "child");
  assert.equal((await step("search_result_opens", cursor)).kind, "pending");
  assert.deepEqual(
    search.input.dispatchedEvents.map((event) => event.key ?? event.type),
    ["input", "Enter"],
  );
  // No results yet: nothing to click, still pending.
  assert.equal((await step("search_result_opens", cursor)).kind, "pending");
  assert.equal(search.result.clicks, 0);
  tree.setElements(".search-match-btn", [search.result]);
  assert.equal((await step("search_result_opens", cursor)).kind, "pending");
  assert.equal(search.result.clicks, 1);
  return search;
}

/** The app menu trigger, the menu, and its New File item (by launch id). */
function installAppMenu(tree, launches = null) {
  const trigger = new FakeElement();
  const newFile = new FakeElement()
    .withAttribute("data-source-focus-launch", "appbar-new")
    .withAttribute("role", "menuitem");
  trigger.onClick = () => {
    tree.setElement("#app-overflow-menu", new FakeElement());
    tree.setElements('[data-source-focus-launch="appbar-new"]', launches ?? [newFile]);
  };
  tree.setElement("#app-menu-trigger", trigger);
  return { trigger, newFile };
}

async function driveToNewFile(tree) {
  const cursor = await driveThroughEnterOpens(tree);
  await driveSearchToActivation(tree, cursor);
  tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 8 }));
  const report = await step("search_result_opens", cursor);
  assert.equal(report.kind, "progress");
  return cursor;
}

/** The Form tab (by launch id; `aria-selected` flips on click, as
 * mode_tabs.rs re-renders it) and the header's `.file-name`, reusing the
 * search fixture already installed for contract (a). */
function installForm(tree) {
  const tab = new FakeElement()
    .withAttribute("data-source-focus-launch", "mode-form")
    .withAttribute("aria-selected", "false");
  tab.onClick = () => tab.withAttribute("aria-selected", "true");
  tree.setElements('[data-source-focus-launch="mode-form"]', [tab]);
  const fileName = new FakeElement({ textContent: "a.md" });
  tree.setElement(".file-name", fileName);
  const trigger = document.querySelector("#workspace-search-trigger");
  const input = document.querySelector("#workspace-search-input");
  const result = document.querySelectorAll(".search-match-btn")[0];
  return { tab, fileName, search: { trigger, input, result } };
}

async function driveToForm(tree) {
  const cursor = await driveToTreeEnter(tree);
  tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 4 }));
  await step("tree_enter_after_new_file", cursor);
  tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 4 }));
  assert.equal((await step("tree_enter_after_new_file", cursor)).kind, "progress");
  return cursor;
}

async function driveToTreeEnter(tree) {
  const cursor = await driveToNewFile(tree);
  installAppMenu(tree);
  assert.equal((await step("new_file_focuses", cursor)).kind, "pending");
  tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 0 }));
  assert.equal((await step("new_file_focuses", cursor)).kind, "progress");
  return cursor;
}

test(
  "task 016: search result, New File, tree Enter, then a Form-mode search; terminal success with all ten milestones",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveThroughEnterOpens(tree);

    const search = await driveSearchToActivation(tree, cursor);
    // The previous document's editor is not the one being waited for.
    tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 4 }));
    assert.equal((await step("search_result_opens", cursor)).kind, "pending");
    tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 8 }));
    const searched = await step("search_result_opens", cursor);
    assert.equal(searched.kind, "progress");
    assert.equal(searched.milestone, "search_result_editor_focused");
    assert.equal(search.trigger.clicks, 1);

    const menu = installAppMenu(tree);
    assert.equal((await step("new_file_focuses", cursor)).kind, "pending");
    assert.equal(menu.trigger.clicks, 1);
    assert.equal(menu.newFile.clicks, 1);
    // Still showing child.md: not yet the untitled document.
    assert.equal((await step("new_file_focuses", cursor)).kind, "pending");
    tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 0 }));
    const created = await step("new_file_focuses", cursor);
    assert.equal(created.kind, "progress");
    assert.equal(created.milestone, "new_file_editor_focused");

    tree.openedPath = null;
    tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 0 }));
    assert.equal((await step("tree_enter_after_new_file", cursor)).kind, "pending");
    assert.equal(tree.activeIndex, 2);
    assert.equal(tree.openedPath, "a.md");
    assert.equal((await step("tree_enter_after_new_file", cursor)).kind, "pending");
    tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 4 }));
    const refocused = await step("tree_enter_after_new_file", cursor);
    assert.equal(refocused.kind, "progress");
    assert.equal(refocused.milestone, "tree_enter_refocused_after_new_file");

    const form = installForm(tree);
    // Step 0: activates Form by its launch id.
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    assert.equal(form.tab.clicks, 1);
    // Step 1: Form selected -> opens search and types the query.
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    assert.equal(form.search.trigger.clicks, 2, "search opened again, once for (a), once here");
    assert.equal(form.search.input.value, "child");
    // Step 2: Enter. Step 3: result appears and is clicked.
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    tree.setElements(".search-match-btn", [form.search.result]);
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    assert.equal(form.search.result.clicks, 2);
    // Step 4: document opened but focus is not on the trigger -> pending.
    form.fileName.textContent = "child.md";
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    // Focus on the trigger but the document not yet opened -> still pending.
    form.fileName.textContent = "a.md";
    Object.defineProperty(document, "activeElement", {
      configurable: true,
      get: () => form.search.trigger,
    });
    assert.equal((await step("form_search_restores", cursor)).kind, "pending");
    form.fileName.textContent = "child.md";
    const formSearched = await step("form_search_restores", cursor);

    assert.equal(formSearched.kind, "progress");
    assert.equal(formSearched.milestone, "form_search_restored_to_trigger");
    assert.equal(window.__bkWebViewShellBehaviourState.phase, "app_menu_mouse_open");

    // Slice 2: the app menu's mouse-open phase, then both overflow menus,
    // three phases each, ending terminal.
    restoreTreeFocus(tree);
    const menus = installMenus(tree);
    const mouse = await step("app_menu_mouse_open", cursor);
    assert.equal(mouse.kind, "progress");
    assert.equal(mouse.milestone, "app_menu_mouse_open_kept_focus");
    const menuReport = await driveMenus(cursor);
    assert.equal(menuReport.kind, "progress");
    assert.equal(menuReport.milestone, "tools_menu_focus_leave_kept");

    // Slice 3 stage 1: mode tabs and focus authority, ending terminal.
    const tabs = new FakeModeTabs(tree).install();
    const d2 = await driveTabsAndAuthority(cursor);
    assert.equal(d2.kind, "progress");
    assert.equal(d2.milestone, "authority_released_editor_refocused");

    // RFC-047 §7: Preview and Text, one exchange.
    const queued = await step("queued_switch_claims_focus", cursor);
    assert.equal(queued.kind, "progress");
    assert.equal(queued.milestone, "queued_switch_focused_editor");

    // Slice 3 stage 2: Settings, then the conflict banner, ending terminal.
    const settings = new FakeSettings(tree, { menu: menus.app, tabs }).install();
    const conflict = new FakeConflict(tree, { tabs }).install();
    assert.equal((await stepToResult("settings_entry", cursor)).kind, "progress");
    assert.equal((await stepToResult("settings_exit_restored", cursor)).kind, "progress");
    assert.equal((await stepToResult("conflict_dirtied", cursor)).kind, "progress");
    conflict.showBanner(); // the Rust sequence's on-disk write, then the poll
    const report = await stepToResult("conflict_banner_focus_kept", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "conflict_banner_focus_kept");
    assert.equal(settings.save.clicks, 0, "Settings' Save is never clicked");
    assert.equal(conflict.actions.reduce((sum, action) => sum + action.clicks, 0), 0, "no banner action");
    assert.equal(menus.app.activations + menus.tools.activations, 0, "no menu item is ever activated");
    assert.equal(tabs.selected, "mode-text");
    assert.deepEqual(report.result.milestones, [
      "recovery_heading_focused",
      "recovery_exit_restored_logo",
      "down_up_moved",
      "expand_entered",
      "collapse_ascended",
      "home_end_reached",
      "non_openable_reachable",
      "enter_opened_editor_focused",
      "search_result_editor_focused",
      "new_file_editor_focused",
      "tree_enter_refocused_after_new_file",
      "form_search_restored_to_trigger",
      "app_menu_mouse_open_kept_focus",
      "app_menu_keys_verified",
      "app_menu_escape_restored",
      "app_menu_focus_leave_kept",
      "tools_menu_keys_verified",
      "tools_menu_escape_restored",
      "tools_menu_focus_leave_kept",
      "tabs_arrows_moved_focus_only",
      "tabs_click_focused_editor",
      "menu_closed_into_editor_kept",
      "authority_released_editor_refocused",
      "queued_switch_focused_editor",
      "settings_heading_focused",
      "settings_exit_restored_trigger",
      "conflict_document_dirtied",
      "conflict_banner_focus_kept",
    ]);
  },
);

test(
  "search_result_opens: focus restored to the search trigger is not success, even with the new document mounted",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveThroughEnterOpens(tree);
    const search = await driveSearchToActivation(tree, cursor);
    tree.setEditorView(new FakeEditorView({ hasFocus: true, docLength: 8 }));
    Object.defineProperty(document, "activeElement", {
      configurable: true,
      get: () => search.trigger,
    });

    const report = await step("search_result_opens", cursor);

    assert.equal(report.kind, "pending");
  },
);

test(
  "search_result_opens: focus that never reaches the editor times out naming the phase and the result count",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveThroughEnterOpens(tree);
    await driveSearchToActivation(tree, cursor);
    tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 8 }));
    tree.setTime(window.__bkWebViewShellBehaviourState.deadline);

    const report = await step("search_result_opens", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "search_result_opens");
    assert.match(
      report.result.error,
      /timed out at search_result_opens \(step 3\): results=1 view=true .*hasFocus=false docLength=8/,
    );
  },
);

test(
  "new_file_focuses: New File is activated only by a unique launch id -- two matches fail and nothing is clicked",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveToNewFile(tree);
    const duplicate = new FakeElement()
      .withAttribute("data-source-focus-launch", "appbar-new")
      .withAttribute("role", "menuitem");
    const other = new FakeElement()
      .withAttribute("data-source-focus-launch", "appbar-new")
      .withAttribute("role", "menuitem");
    installAppMenu(tree, [duplicate, other]);

    const report = await step("new_file_focuses", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /expected exactly one \[data-source-focus-launch="appbar-new"\], found 2/);
    assert.equal(duplicate.clicks + other.clicks, 0);
  },
);

test(
  "new_file_focuses: an editor that never takes focus times out naming the phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveToNewFile(tree);
    installAppMenu(tree);
    await step("new_file_focuses", cursor);
    tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 0 }));
    tree.setTime(window.__bkWebViewShellBehaviourState.deadline);

    const report = await step("new_file_focuses", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /timed out at new_file_focuses: view=true .*hasFocus=false/);
  },
);

test(
  "tree_enter_after_new_file: focus left on the row times out naming the phase and the row",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveToTreeEnter(tree);
    tree.setEditorView(new FakeEditorView({ hasFocus: false, docLength: 4 }));
    await step("tree_enter_after_new_file", cursor);
    tree.setTime(window.__bkWebViewShellBehaviourState.deadline);

    const report = await step("tree_enter_after_new_file", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "tree_enter_after_new_file");
    assert.match(
      report.result.error,
      /timed out at tree_enter_after_new_file: view=true .*hasFocus=false .*activeElement=tree row 2/,
    );
  },
);

test(
  "form_search_restores: a result that leaves focus off the trigger times out naming the phase, the file and activeElement",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveToForm(tree);
    const form = installForm(tree);
    await step("form_search_restores", cursor);
    await step("form_search_restores", cursor);
    await step("form_search_restores", cursor);
    await step("form_search_restores", cursor);
    form.fileName.textContent = "child.md";
    tree.setTime(window.__bkWebViewShellBehaviourState.deadline);

    const report = await step("form_search_restores", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "form_search_restores");
    assert.match(
      report.result.error,
      /timed out at form_search_restores \(step 4\): formSelected=true fileName=child\.md results=1 activeElement=tree row 2/,
    );
  },
);

test(
  "form_search_restores: Form is activated only by a unique launch id -- two matches fail and nothing is clicked",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const cursor = await driveToForm(tree);
    const form = installForm(tree);
    const other = new FakeElement().withAttribute("data-source-focus-launch", "mode-form");
    tree.setElements('[data-source-focus-launch="mode-form"]', [form.tab, other]);

    const report = await step("form_search_restores", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /expected exactly one \[data-source-focus-launch="mode-form"\], found 2/);
    assert.equal(form.tab.clicks + other.clicks, 0);
  },
);

// ---- slice 2: overflow menus ---------------------------------------------

/** The form phase leaves `document.activeElement` pinned to the search
 * trigger; the menu phases need the tree's own focus back. */
function restoreTreeFocus(tree) {
  Object.defineProperty(document, "activeElement", {
    configurable: true,
    get: () => tree.activeElement(),
  });
  tree.setFocus(tree.elements()[2]);
}

function installMenus(tree, options = {}) {
  const app = new FakeMenu(tree, {
    triggerSelector: "#app-menu-trigger",
    menuSelector: "#app-overflow-menu",
    ...(options.app ?? {}),
  }).install();
  const tools = new FakeMenu(tree, {
    triggerSelector: "#editor-tools-trigger",
    menuSelector: "#editor-tools-menu",
    ...(options.tools ?? {}),
  }).install();
  return { app, tools };
}

const MENU_PHASE_ORDER = [
  "app_menu_keys",
  "app_menu_escape",
  "app_menu_focus_leave",
  "tools_menu_keys",
  "tools_menu_escape",
  "tools_menu_focus_leave",
];

/** Runs the six menu phases in order, stopping early on a terminal report. */
async function driveMenus(cursor, upTo = MENU_PHASE_ORDER.length) {
  let report;
  for (const phase of MENU_PHASE_ORDER.slice(0, upTo)) {
    report = await step(phase, cursor);
    if (report.kind === "terminal") return report;
  }
  return report;
}

/** Drives every earlier phase, then hands back a cursor at app_menu_keys. */
async function driveToMenus(tree, menuOptions) {
  const cursor = await driveToForm(tree);
  const form = installForm(tree);
  await step("form_search_restores", cursor); // activate Form
  await step("form_search_restores", cursor); // open search, type
  await step("form_search_restores", cursor); // Enter
  tree.setElements(".search-match-btn", [form.search.result]);
  await step("form_search_restores", cursor); // activate the result
  form.fileName.textContent = "child.md";
  Object.defineProperty(document, "activeElement", {
    configurable: true,
    get: () => form.search.trigger,
  });
  const searched = await step("form_search_restores", cursor);
  assert.equal(searched.kind, "progress");
  restoreTreeFocus(tree);
  const menus = installMenus(tree, menuOptions);
  if (!menuOptions?.stopBeforeMouseOpen) {
    const mouse = await step("app_menu_mouse_open", cursor);
    assert.equal(mouse.kind, "progress", "the mouse-open phase precedes the key phases");
  }
  return { cursor, menus };
}

test(
  "app_menu_keys: Down/Up/Enter/Space open to the right edge, Down/Up wrap, Home/End jump, and no item is activated",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);
    const before = menus.app.trigger.dispatchedEvents.length;

    const report = await step("app_menu_keys", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "app_menu_keys_verified");
    assert.equal(menus.app.open, false, "the phase leaves the menu closed");
    assert.equal(document.activeElement, menus.app.trigger);
    assert.equal(menus.app.activations, 0);
    const keys = menus.app.trigger.dispatchedEvents.slice(before).map((event) => event.key);
    assert.deepEqual(keys, ["ArrowDown", "ArrowUp", "Enter", " "], "contracts 1-3, on the trigger");
  },
);

test(
  "app_menu_escape: Escape closes the menu and restores focus to the trigger",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);
    await step("app_menu_keys", cursor);

    const report = await step("app_menu_escape", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "app_menu_escape_restored");
    assert.equal(menus.app.open, false);
    assert.equal(document.activeElement, menus.app.trigger);
  },
);

test(
  "app_menu_focus_leave: focus moving to the tree row closes the menu and is NOT restored to the trigger",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);
    await driveMenus(cursor, 2);

    const report = await step("app_menu_focus_leave", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "app_menu_focus_leave_kept");
    assert.equal(menus.app.open, false);
    assert.equal(document.activeElement, tree.elements()[2], "focus stays where it went");
    assert.notEqual(document.activeElement, menus.app.trigger);
  },
);

test(
  "contract 4: a menu whose Down does not wrap fails, naming the key and the menu state",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToMenus(tree, { app: { wraps: false } });

    const report = await step("app_menu_keys", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "app_menu_keys");
    assert.match(
      report.result.error,
      /app_menu: ArrowDown to reach the first item \(at dispatch: menu=true expanded=true items=3 .*; at timeout: menu=true expanded=true items=3 activeElement=/,
    );
  },
);

test(
  "contract 6: Escape that releases without restoring fails, naming the phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);
    // Only after the mouse-open phase, which itself closes with Escape: this
    // test targets contract 6 in the key phase.
    menus.app.restoreOnEscape = false;

    const report = await step("app_menu_keys", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(report.result.error, /app_menu: Escape to close and restore focus to the trigger/);
  },
);

test(
  "contract 7: a menu that restores focus on focus-leave fails, naming implicit dismissal",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToMenus(tree, { app: { restoreOnFocusLeave: true } });
    await driveMenus(cursor, 2);

    const report = await step("app_menu_focus_leave", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "app_menu_focus_leave");
    assert.match(
      report.result.error,
      /app_menu: (focus leaving to the roving tree row to close the menu|focus was restored to the trigger; implicit dismissal must not restore)/,
    );
  },
);

test(
  "the editor-tools menu is driven through the same three phases, then hands over to the mode tabs",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);

    const report = await driveMenus(cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "tools_menu_focus_leave_kept");
    assert.equal(window.__bkWebViewShellBehaviourState.phase, "tabs_arrows_focus_only");
    assert.equal(menus.tools.open, false);
    assert.equal(menus.tools.activations, 0);
    assert.deepEqual(
      menus.tools.trigger.dispatchedEvents.map((event) => event.key),
      ["ArrowDown", "ArrowUp", "Enter", " ", "ArrowDown", "ArrowDown"],
      "contracts 1-3 in the keys phase, then one open each for escape and focus-leave",
    );
  },
);

test(
  "app_menu_mouse_open: a mouse open leaves focus on the trigger, alone and after a keyboard open and close",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree, { stopBeforeMouseOpen: true });

    const report = await step("app_menu_mouse_open", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "app_menu_mouse_open_kept_focus");
    assert.equal(menus.app.trigger.clicks, 2, "one mouse open alone, one after the keyboard round trip");
    assert.equal(menus.app.open, false);
    assert.equal(document.activeElement, menus.app.trigger);
    assert.equal(menus.app.activations, 0);
  },
);

test(
  "app_menu_mouse_open: a leftover entry intent that moves focus into the menu fails, naming the phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToMenus(tree, {
      stopBeforeMouseOpen: true,
      app: { leftoverIntent: true },
    });

    const report = await step("app_menu_mouse_open", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "app_menu_mouse_open");
    assert.match(report.result.error, /app_menu: a mouse open \(alone\) moved focus into the menu/);
  },
);

test(
  "§4.1: a menu timeout describes the state at the timeout, not only at dispatch",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree, { app: { wraps: false } });
    // Make the menu close itself after the key: the dispatch-time snapshot
    // still shows it open; only the timeout-time one can show it gone.
    const realItemKey = menus.app.handleItemKey.bind(menus.app);
    menus.app.handleItemKey = (item, key) => {
      realItemKey(item, key);
      if (key === "ArrowDown") queueMicrotask(() => menus.app.setOpen(false));
    };

    const report = await step("app_menu_keys", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(
      report.result.error,
      /at dispatch: menu=true expanded=true items=3 .*; at timeout: menu=false expanded=false items=0/,
    );
  },
);

test(
  "§4.2: a tabindex that follows focus by a few frames settles and passes",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    // Longer than one frame (16 ms), far shorter than the 2 s deadline.
    tree.tabindexLagMs = 50;

    const report = await exchange("down_up", 1);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "down_up_moved");
  },
);

test(
  "contract 7: a restore that lands one frame after the menu is removed still fails, naming implicit dismissal",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree, { app: { restoreOnFocusLeave: "nextFrame" } });
    await driveMenus(cursor, 2);

    const report = await step("app_menu_focus_leave", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "app_menu_focus_leave");
    assert.match(
      report.result.error,
      /app_menu: focus was restored to the trigger; implicit dismissal must not restore/,
    );
    assert.equal(document.activeElement, menus.app.trigger, "the fake did restore, one frame late");
  },
);

// ---- slice 3 stage 1: mode tabs (§8 C) and focus authority (§8 D) ---------

const STAGE_1_PHASES = [
  "tabs_arrows_focus_only",
  "tabs_click_activates",
  "menu_closes_into_editor",
  "authority_released_after_editor_focus",
];

/** Runs stage 1's phases in order, stopping early on a terminal report. D2
 * takes two exchanges (task 021), so each phase is stepped to its result. */
async function driveTabsAndAuthority(cursor, upTo = STAGE_1_PHASES.length) {
  let report;
  for (const phase of STAGE_1_PHASES.slice(0, upTo)) {
    report = await stepToResult(phase, cursor);
    if (report.kind === "terminal") return report;
  }
  return report;
}

/** Drives every earlier phase, then hands back a cursor at stage 1's start,
 * with the Form tab selected and focus on a tree row -- slice 2's end state. */
async function driveToTabs(tree, { menus: menuOptions, tabs: tabOptions } = {}) {
  const { cursor, menus } = await driveToMenus(tree, menuOptions);
  const last = await driveMenus(cursor);
  assert.equal(last.kind, "progress", "slice 2's last phase hands over to stage 1");
  const tabs = new FakeModeTabs(tree, tabOptions).install();
  return { cursor, menus, tabs };
}

test(
  "C1 tabs_arrows_focus_only: Right, Left, Home and End move focus only; Form stays selected",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);

    const report = await step("tabs_arrows_focus_only", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "tabs_arrows_moved_focus_only");
    assert.equal(tabs.selected, "mode-form");
    assert.equal(document.activeElement, tabs.tab("mode-form"));
    const keys = tabs.elements.flatMap((tab) => tab.dispatchedEvents.map((event) => event.key));
    assert.deepEqual(keys.sort(), ["ArrowLeft", "ArrowRight", "End", "Home"]);
    assert.equal(tabs.elements.reduce((sum, tab) => sum + tab.clicks, 0), 0, "C1 clicks nothing");
  },
);

test(
  "C1: a tab that activates one frame after the arrow key fails, naming the selection change",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToTabs(tree, { tabs: { activateOnArrow: "nextFrame" } });

    const report = await step("tabs_arrows_focus_only", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "tabs_arrows_focus_only");
    assert.match(
      report.result.error,
      /C1: ArrowRight changed the selected tab, not only focus: the selected tab is mode-text, expected mode-form/,
    );
  },
);

test(
  "C2 tabs_click_activates: activating Text selects it and the editor takes focus",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 1);

    const report = await step("tabs_click_activates", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "tabs_click_focused_editor");
    assert.equal(tabs.selected, "mode-text");
    assert.equal(tabs.tab("mode-text").clicks, 1);
    assert.equal(document.activeElement, tabs.content);
  },
);

test(
  "C2: a refused editor claim times out, naming C2 and the tab and editor state",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 1);
    tabs.authorityHeld = true;

    const report = await step("tabs_click_activates", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.match(
      report.result.error,
      /C2: activating the Text tab to select it and focus the editor \(at timeout: tabs=\[mode-text:selected=true.* hasFocus=false/,
    );
  },
);

test(
  "D1 menu_closes_into_editor: focus entering the editor closes the menu and stays in the editor",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 2);

    const report = await step("menu_closes_into_editor", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "menu_closed_into_editor_kept");
    assert.equal(menus.app.open, false);
    assert.equal(document.activeElement, tabs.content);
    assert.equal(tabs.view.hasFocus, true);
  },
);

test(
  "D1: a restore to the trigger one frame after the editor takes focus fails, naming implicit dismissal",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 2);
    menus.app.restoreOnFocusLeave = "nextFrame";

    const report = await step("menu_closes_into_editor", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "menu_closes_into_editor");
    assert.match(
      report.result.error,
      /app_menu: focus was restored to the trigger; implicit dismissal must not restore/,
    );
  },
);

test(
  "D2 authority_released_after_editor_focus: a later claim succeeds, then hands over to the queued-switch phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 3);

    const report = await stepToResult("authority_released_after_editor_focus", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "authority_released_editor_refocused");
    assert.equal(window.__bkWebViewShellBehaviourState.phase, "queued_switch_claims_focus");
    assert.equal(tabs.tab("mode-preview").clicks, 1);
    assert.equal(document.activeElement, tabs.content);
  },
);

test(
  "D2: authority still held after the close into the editor fails at D2, naming the refused claim",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor, 3);
    // The defect D1 cannot see: the menu closed, but authority stayed held.
    tabs.authorityHeld = true;

    const report = await stepToResult("authority_released_after_editor_focus", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "authority_released_after_editor_focus");
    assert.match(
      report.result.error,
      /D2: activating Text to claim the editor again; a refused claim means the menu's close into the editor kept shell authority/,
    );
  },
);

// ---- task 021: D2 takes two exchanges, so the Rust gate can sit between them

test(
  "D2 shape: Preview and Text are never clicked in the same exchange",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree, { tabs: { teardownLag: true } });
    await driveTabsAndAuthority(cursor, 3);
    const textClicksBefore = tabs.tab("mode-text").clicks;

    const first = await step("authority_released_after_editor_focus", cursor);

    assert.equal(first.kind, "pending", "the Preview exchange returns pending");
    assert.equal(tabs.tab("mode-preview").clicks, 1, "Preview was clicked");
    assert.equal(tabs.selected, "mode-preview");
    assert.equal(
      tabs.tab("mode-text").clicks,
      textClicksBefore,
      "Text was not clicked in the Preview exchange",
    );

    // The Rust sequence requests the next exchange only once the controller
    // has settled; the fake's counterpart is the destroyed event.
    tabs.finishTeardown();
    const second = await step("authority_released_after_editor_focus", cursor);

    assert.equal(second.kind, "progress");
    assert.equal(second.milestone, "authority_released_editor_refocused");
    assert.equal(tabs.tab("mode-text").clicks, textClicksBefore + 1, "Text in the later exchange");
    assert.equal(tabs.droppedActivations, 0);
    assert.equal(document.activeElement, tabs.content);
  },
);

test(
  "D2 reproduction: a Text activation inside the teardown window is dropped, and D2 fails as CI did",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree, { tabs: { teardownLag: true } });
    await driveTabsAndAuthority(cursor, 3);

    await step("authority_released_after_editor_focus", cursor);
    // No finishTeardown(): what the driver alone would do without the gate.
    const report = await step("authority_released_after_editor_focus", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(tabs.droppedActivations, 1, "the click reached Busy and was dropped");
    assert.match(
      report.result.error,
      /D2: activating Text to claim the editor again.*mode-text:selected=false.*mode-preview:selected=true/,
      "the failure names Preview still selected, as run 35168125047 did",
    );
  },
);

// ---- RFC-047 §7: Preview and Text clicked with nothing waiting between them

test(
  "G1 queued_switch_claims_focus: Text wins and takes focus when clicked right after Preview, one exchange",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    // No lag through D2: it must complete normally, as every other test
    // reaching this point does. The lag is switched on only for this
    // phase's own exchange, not retroactively for D2's.
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor);
    tabs.teardownLag = true;
    tabs.queueDuringTeardown = true;
    // The teardown ends partway through the phase's own waitFor loop, exactly
    // as the real destroyed event can arrive mid-poll -- not between exchanges,
    // since this phase is deliberately one exchange only. `setImmediate`, not
    // a real timer: the fake clock's `requestAnimationFrame` (tree-fake.mjs)
    // is itself `setImmediate`-driven, so this interleaves with its polling
    // deterministically instead of racing a real wall-clock delay against it.
    setImmediate(() => tabs.finishTeardown());

    const report = await step("queued_switch_claims_focus", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "queued_switch_focused_editor");
    assert.equal(tabs.selected, "mode-text");
    assert.equal(tabs.droppedActivations, 0, "nothing was dropped: it was queued");
    assert.equal(document.activeElement, tabs.content);
  },
);

test(
  "G1 mutation: reverting to the pre-RFC-047 drop fails the phase, naming itself",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, tabs } = await driveToTabs(tree);
    await driveTabsAndAuthority(cursor);
    // `queueDuringTeardown` left at its default (false): the old Busy-drop
    // shape, switched on only for this phase's own exchange.
    tabs.teardownLag = true;

    const report = await step("queued_switch_claims_focus", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, false);
    assert.equal(report.result.stage, "queued_switch_claims_focus");
    assert.equal(tabs.droppedActivations, 1, "the click reached Busy and was dropped, as before RFC-047");
    assert.match(
      report.result.error,
      /G1: Text must win and the editor must take focus/,
      "the phase fails naming itself, not a generic timeout",
    );
  },
);

// ---- slice 3 stage 2: Settings and Recovery (§8 E), conflict banner (§8 F) --

async function driveRecovery(tree, options) {
  tree.skipRecoveryPreamble = true;
  const recovery = new FakeRecovery(tree, options).install();
  const cursor = { exchangeId: 1, release: null };
  return { recovery, cursor };
}

test(
  "E1 recovery_entry: the Recovery screen shows one snapshot with focus on its heading",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { recovery, cursor } = await driveRecovery(tree);

    const report = await stepToResult("recovery_entry", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "recovery_heading_focused");
    assert.equal(document.activeElement, recovery.heading);
    assert.equal(recovery.skipped, 0);
  },
);

test(
  "E1: entry focus that never reaches the heading times out, naming E1",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveRecovery(tree, { focusHeading: false });

    const report = await stepToResult("recovery_entry", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.stage, "recovery_entry");
    assert.match(report.result.error, /E1: the Recovery screen with focus on #recovery-heading \(at timeout: recovery=true/);
  },
);

test(
  "E1: a status that does not report one snapshot fails, quoting it",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveRecovery(tree, { count: 2 });

    const report = await stepToResult("recovery_entry", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(report.result.error, /E1: the Recovery status does not report one snapshot: "2 recoverable"/);
  },
);

test(
  "E2 recovery_exit: Skip all closes Recovery and focus stays on the app-bar logo",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { recovery, cursor } = await driveRecovery(tree);
    await stepToResult("recovery_entry", cursor);

    const report = await stepToResult("recovery_exit", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "recovery_exit_restored_logo");
    assert.equal(recovery.skip.clicks, 1, "only Skip all");
    assert.equal(document.activeElement, recovery.logo);
    assert.equal(recovery.logo.clicks, 0, "the logo is never clicked");
  },
);

test(
  "E2: a Recovery exit that does not restore to the logo times out, naming E2",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveRecovery(tree, { restoreToLogo: false });
    await stepToResult("recovery_entry", cursor);

    const report = await stepToResult("recovery_exit", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.stage, "recovery_exit");
    assert.match(report.result.error, /E2: Skip all to close Recovery and restore focus to #app-bar-logo-trigger/);
  },
);

test(
  "E2: focus taken from the logo one frame after Recovery exits fails, naming E2",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveRecovery(tree, { stealFocusNextFrame: true });
    await stepToResult("recovery_entry", cursor);

    const report = await stepToResult("recovery_exit", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(report.result.error, /E2: focus did not stay on #app-bar-logo-trigger after Recovery closed/);
  },
);

/** Drives every phase through D2, then installs Settings and the conflict
 * banner: Text mode, editor focused -- stage 1's end state. */
async function driveToSettings(tree, { settings: settingsOptions, conflict: conflictOptions } = {}) {
  const { cursor, menus, tabs } = await driveToTabs(tree);
  const d2 = await driveTabsAndAuthority(cursor);
  assert.equal(d2.kind, "progress", "stage 1 hands over to stage 2");
  // RFC-047 §7: Preview and Text, one exchange, no lag configured here, so it
  // resolves within this one step and hands straight to Settings.
  const queued = await step("queued_switch_claims_focus", cursor);
  assert.equal(queued.kind, "progress", "the queued-switch phase hands over to Settings");
  const settings = new FakeSettings(tree, { menu: menus.app, tabs, ...settingsOptions }).install();
  const conflict = new FakeConflict(tree, { tabs, ...conflictOptions }).install();
  return { cursor, menus, tabs, settings, conflict };
}

test(
  "E3 settings_entry: Settings opens by its id from the keyboard-opened app menu, focus on its heading",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus, settings } = await driveToSettings(tree);

    const report = await stepToResult("settings_entry", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "settings_heading_focused");
    assert.equal(settings.item.clicks, 1);
    assert.equal(menus.app.activations, 0, "no other menu item");
    assert.equal(document.activeElement, settings.heading);
  },
);

test(
  "E3: a Settings handle outside the app menu is refused, and nothing is clicked",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus, settings } = await driveToSettings(tree);
    menus.app.extraContained.length = 0;

    const report = await stepToResult("settings_entry", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(
      report.result.error,
      /E3: refusing to click: expected exactly one #app-menu-settings inside #app-overflow-menu, found 1 \(inside the menu: false\)/,
    );
    assert.equal(settings.item.clicks, 0);
  },
);

test(
  "E4 settings_exit_restored: Close leaves Settings, the editor remounts, focus stays on the trigger",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus, settings } = await driveToSettings(tree);
    await stepToResult("settings_entry", cursor);

    const report = await stepToResult("settings_exit_restored", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "settings_exit_restored_trigger");
    assert.equal(settings.close.clicks, 1);
    assert.equal(settings.save.clicks, 0, "never Save");
    assert.equal(document.activeElement, menus.app.trigger);
  },
);

test(
  "E4: a Settings exit that does not restore to the trigger times out, naming E4",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToSettings(tree, { settings: { restoreToTrigger: false } });
    await stepToResult("settings_entry", cursor);

    const report = await stepToResult("settings_exit_restored", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.stage, "settings_exit_restored");
    assert.match(report.result.error, /E4: Close to leave Settings, remount the editor and restore focus to #app-menu-trigger/);
  },
);

test(
  "E4: the remounted editor taking focus one frame after the restore fails, naming E4",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToSettings(tree, { settings: { editorTakesFocusNextFrame: true } });
    await stepToResult("settings_entry", cursor);

    const report = await stepToResult("settings_exit_restored", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(report.result.error, /E4: focus did not stay on #app-menu-trigger after Settings closed/);
  },
);

test(
  "F1 conflict_dirtied: one insertion into child.md, then the header's dirty dot",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, conflict, tabs } = await driveToSettings(tree);
    await stepToResult("settings_entry", cursor);
    await stepToResult("settings_exit_restored", cursor);

    const report = await stepToResult("conflict_dirtied", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "conflict_document_dirtied");
    assert.equal(conflict.dispatches, 1);
    assert.equal(document.activeElement, tabs.content);
  },
);

test(
  "F1: a different open document fails before any edit, naming the file",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, conflict } = await driveToSettings(tree, { conflict: { fileName: "a.md" } });
    await stepToResult("settings_entry", cursor);
    await stepToResult("settings_exit_restored", cursor);

    const report = await stepToResult("conflict_dirtied", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(report.result.error, /F1: the open document is a\.md, not child\.md/);
    assert.equal(conflict.dispatches, 0);
  },
);

async function driveToBanner(tree, conflictOptions) {
  const drive = await driveToSettings(tree, { conflict: conflictOptions });
  await stepToResult("settings_entry", drive.cursor);
  await stepToResult("settings_exit_restored", drive.cursor);
  await stepToResult("conflict_dirtied", drive.cursor);
  drive.conflict.showBanner();
  return drive;
}

test(
  "F2 conflict_banner_focus_kept: the three-action banner appears and focus stays in the editor",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, conflict, tabs } = await driveToBanner(tree);

    const report = await stepToResult("conflict_banner_focus_kept", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "conflict_banner_focus_kept");
    assert.equal(report.result.milestones.length, 28);
    assert.equal(document.activeElement, tabs.content);
    assert.equal(conflict.actions.reduce((sum, action) => sum + action.clicks, 0), 0);
  },
);

test(
  "F2: a two-action banner (a deleted file, not dirty memory) fails, naming the count",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToBanner(tree, { buttons: 2 });

    const report = await stepToResult("conflict_banner_focus_kept", cursor);

    assert.equal(report.kind, "terminal");
    assert.match(report.result.error, /F2: expected the dirty-memory banner's three actions, found 2/);
  },
);

test(
  "F2: a banner that focuses its first action one frame after it appears fails, naming F2",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToBanner(tree, { focusFirstActionNextFrame: true });

    const report = await stepToResult("conflict_banner_focus_kept", cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.stage, "conflict_banner_focus_kept");
    assert.match(report.result.error, /F2: focus moved into the conflict banner/);
  },
);

test(
  "stage 2 phases never hold one exchange past the transport's 5 s evaluator timeout while they wait",
  { concurrency: false },
  async () => {
    // A missing restore must fail with the contract's own message, which
    // only reaches the log if every exchange returns before the transport
    // gives up on it (35157442989: "recovery_exit phase evaluator did not
    // report progress").
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveRecovery(tree, { restoreToLogo: false });
    await stepToResult("recovery_entry", cursor);
    for (let call = 0; call < 4; call += 1) {
      const before = performance.now();
      const report = await step("recovery_exit", cursor);
      assert.equal(report.kind, "pending");
      assert.ok(
        performance.now() - before < TRANSPORT_EVALUATOR_BUDGET_MS,
        `recovery_exit exchange ${call} held ${performance.now() - before} ms`,
      );
    }

    const settingsTree = new FakeTree();
    settingsTree.install();
    const drive = await driveToSettings(settingsTree, { settings: { restoreToTrigger: false } });
    await stepToResult("settings_entry", drive.cursor);
    for (let call = 0; call < 4; call += 1) {
      const before = performance.now();
      const report = await step("settings_exit_restored", drive.cursor);
      assert.equal(report.kind, "pending");
      assert.ok(
        performance.now() - before < TRANSPORT_EVALUATOR_BUDGET_MS,
        `settings_exit_restored exchange ${call} held ${performance.now() - before} ms`,
      );
    }
  },
);

test(
  "an unrecognized phase name sends a named diagnostic instead of hanging silently",
  { concurrency: false },
  async () => {
    // Task 025 §2.3/§4.4: the same property task 023's failEarly proved for
    // trusted_click_driver.js (trusted-click-driver.test.mjs), now here too
    // -- a rejection before the try sends a diagnostic terminal report over
    // the channel first, so Rust's own eval.recv() never waits out the
    // shared transport's 5 s cap in silence. RFC-044's own terminal arm
    // (ConflictBannerFocusKept => TERMINAL_STAGE, task 025 §2.1) was this
    // exact latent bug: this test would have caught a phase-name/stage-name
    // mismatch at the JS boundary, had that mismatch ever reached the wire.
    const tree = new FakeTree();
    tree.install();
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

    await assert.rejects(completion);
  },
);
