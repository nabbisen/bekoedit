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
    assert.equal(window.__bkWebViewShellBehaviourState.phase, "app_menu_keys");

    // Slice 2: both overflow menus, three phases each, ending terminal.
    restoreTreeFocus(tree);
    const menus = installMenus(tree);
    const report = await driveMenus(cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "tools_menu_focus_leave");
    assert.equal(menus.app.activations + menus.tools.activations, 0, "no menu item is ever activated");
    assert.deepEqual(report.result.milestones, [
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
      "app_menu_keys_verified",
      "app_menu_escape_restored",
      "app_menu_focus_leave_kept",
      "tools_menu_keys_verified",
      "tools_menu_escape_restored",
      "tools_menu_focus_leave_kept",
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
  return { cursor, menus };
}

test(
  "app_menu_keys: Down/Up/Enter/Space open to the right edge, Down/Up wrap, Home/End jump, and no item is activated",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);

    const report = await step("app_menu_keys", cursor);

    assert.equal(report.kind, "progress");
    assert.equal(report.milestone, "app_menu_keys_verified");
    assert.equal(menus.app.open, false, "the phase leaves the menu closed");
    assert.equal(document.activeElement, menus.app.trigger);
    assert.equal(menus.app.activations, 0);
    const keys = menus.app.trigger.dispatchedEvents.map((event) => event.key);
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
      /app_menu: ArrowDown to reach the first item \(menu=true expanded=true items=3/,
    );
  },
);

test(
  "contract 6: Escape that releases without restoring fails, naming the phase",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor } = await driveToMenus(tree, { app: { restoreOnEscape: false } });

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
  "the editor-tools menu is driven through the same three phases, ending terminal",
  { concurrency: false },
  async () => {
    const tree = new FakeTree();
    tree.install();
    const { cursor, menus } = await driveToMenus(tree);

    const report = await driveMenus(cursor);

    assert.equal(report.kind, "terminal");
    assert.equal(report.result.ok, true);
    assert.equal(report.result.stage, "tools_menu_focus_leave");
    assert.equal(menus.tools.open, false);
    assert.equal(menus.tools.activations, 0);
    assert.deepEqual(
      menus.tools.trigger.dispatchedEvents.map((event) => event.key),
      ["ArrowDown", "ArrowUp", "Enter", " ", "ArrowDown", "ArrowDown"],
      "contracts 1-3 in the keys phase, then one open each for escape and focus-leave",
    );
  },
);
