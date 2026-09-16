return (async () => {
  const marker = "RFC044_SHELL_BEHAVIOUR_MARKER";
  const stateKey = "__bkWebViewShellBehaviourState";
  const pinKey = "__bkWebViewSmokeEvalPin";
  const protocolVersion = 2;
  const pinProtocolVersion = 1;
  const SHELL_REPLACED_AT_START = new Set(["recovery_entry", "recovery_exit", "settings_exit_restored"]);
  const phases = [
    "recovery_entry",
    "recovery_exit",
    "down_up",
    "expand_enter",
    "collapse_ascend",
    "home_end",
    "non_openable",
    "enter_opens",
    "search_result_opens",
    "new_file_focuses",
    "tree_enter_after_new_file",
    "form_search_restores",
    "app_menu_mouse_open",
    "app_menu_keys",
    "app_menu_escape",
    "app_menu_focus_leave",
    "tools_menu_keys",
    "tools_menu_escape",
    "tools_menu_focus_leave",
    "tabs_arrows_focus_only",
    "tabs_click_activates",
    "menu_closes_into_editor",
    "authority_released_after_editor_focus",
    "settings_entry",
    "settings_exit_restored",
    "conflict_dirtied",
    "conflict_banner_focus_kept",
  ];
  const request = await dioxus.recv();
  const requestedPhase = request?.phase;
  const exchangeId = request?.exchangeId;

  if (
    request?.protocolVersion !== protocolVersion ||
    !Number.isSafeInteger(exchangeId) ||
    exchangeId <= 0 ||
    !phases.includes(requestedPhase)
  ) {
    throw new Error("invalid phase request");
  }

  // Pin-registry protocol, identical to driver.js's (RFC-044 slice-1 §3):
  // one canonical shape, kept in sync by
  // webview-smoke-driver-parity.test.mjs rather than by sharing a file --
  // driver.js itself is out of scope for this slice (handoff §9).
  let pinRegistry = window[pinKey];
  if (pinRegistry === undefined) {
    pinRegistry = Object.seal({ protocolVersion: pinProtocolVersion, current: null });
    Object.defineProperty(window, pinKey, {
      value: pinRegistry,
      configurable: false,
      enumerable: false,
      writable: false,
    });
  } else if (
    pinRegistry?.protocolVersion !== pinProtocolVersion ||
    !Object.isSealed(pinRegistry) ||
    Object.keys(pinRegistry).sort().join(",") !== "current,protocolVersion"
  ) {
    throw new Error("incompatible smoke evaluator pin registry");
  }

  const hasReleaseId = request.releaseExchangeId !== null;
  const hasReleasePhase = request.releasePhase !== null;
  if (hasReleaseId !== hasReleasePhase) {
    throw new Error("incomplete prior evaluator pin release");
  }
  let releasedExchangeId = null;
  let releasedPhase = null;
  if (hasReleaseId) {
    if (
      !Number.isSafeInteger(request.releaseExchangeId) ||
      request.releaseExchangeId <= 0 ||
      !phases.includes(request.releasePhase) ||
      pinRegistry.current?.exchangeId !== request.releaseExchangeId ||
      pinRegistry.current?.phase !== request.releasePhase ||
      !pinRegistry.current?.channel
    ) {
      throw new Error("prior evaluator pin did not match release request");
    }
    releasedExchangeId = request.releaseExchangeId;
    releasedPhase = request.releasePhase;
    pinRegistry.current = null;
  } else if (pinRegistry.current !== null) {
    throw new Error("unexpected prior evaluator pin");
  }

  // ---- shell-behaviour state: tree navigation, RFC-044 slice-1 §5 -------

  const rows = () => [...document.querySelectorAll("[data-tree-row]")];
  const dispatchKey = (element, key) => {
    element.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  };
  /** Polls `predicate` with a timeout -- never sleeps as the wait itself,
   * matching the harness's poll-with-timeout discipline (RFC-044 §7). */
  const waitFor = async (predicate, description, timeoutMs = 2000) => {
    const deadline = performance.now() + timeoutMs;
    while (!predicate()) {
      if (performance.now() >= deadline) {
        // A function is called here, at the timeout, so a state report
        // describes the moment of failure rather than the moment of the call.
        const text = typeof description === "function" ? description() : description;
        throw new Error(`timed out waiting for: ${text}`);
      }
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
  };
  /** RFC-044 §8 A.1's corrected mechanism: a synthetic Tab cannot drive
   * focus (untrusted events get no browser default action), so contract 1
   * is this invariant, asserted live after each app-intercepted nav key
   * instead -- exactly one row at tabindex=0, it is the row that just
   * became active, and it is document.activeElement.
   *
   * The row's tabindex and DOM focus arrive by separate routes -- a Dioxus
   * render, and focus_tree_row's script on its next frame -- and nothing
   * orders them. So the invariant is awaited until it settles, and a
   * violation that is still there at the deadline fails with its own named
   * message. A difference lasting a frame is invisible to a keyboard user;
   * one that persists is not. */
  const tabStopViolation = (expectedIndex) => {
    const all = rows();
    const zeroed = all.filter((row) => row.getAttribute("tabindex") === "0");
    if (zeroed.length !== 1) {
      return `roving-tabindex invariant: expected exactly one row at tabindex=0, found ${zeroed.length}`;
    }
    if (all[expectedIndex] !== zeroed[0]) {
      return "roving-tabindex invariant: the tabindex=0 row is not the row that just became active";
    }
    if (document.activeElement !== zeroed[0]) {
      return "roving-tabindex invariant: the tabindex=0 row is not document.activeElement";
    }
    return null;
  };
  const checkTabStopInvariant = async (expectedIndex, timeoutMs = 2000) => {
    const deadline = performance.now() + timeoutMs;
    let violation = tabStopViolation(expectedIndex);
    while (violation !== null) {
      if (performance.now() >= deadline) throw new Error(violation);
      await new Promise((resolve) => requestAnimationFrame(resolve));
      violation = tabStopViolation(expectedIndex);
    }
  };

  const containsErrorToast = (node) =>
    node?.nodeType === Node.ELEMENT_NODE &&
    (node.matches?.(".toast-error") || node.querySelector?.(".toast-error"));

  const createState = () => {
    const state = {
      protocolVersion: 1,
      phase: "recovery_entry",
      stage: "recovery_entry",
      deadline: null,
      step: 0,
      milestones: [],
      errorToastSeen: Boolean(document.querySelector(".toast-error")),
      observer: null,
    };
    state.observer = new MutationObserver((records) => {
      for (const record of records) {
        if (
          [...record.addedNodes].some(containsErrorToast) ||
          containsErrorToast(record.target)
        ) {
          state.errorToastSeen = true;
        }
      }
    });
    state.observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["class"],
    });
    window[stateKey] = state;
    return state;
  };

  const state = window[stateKey] ?? createState();
  const finish = (ok, error = null) => {
    const result = {
      ok,
      stage: state.stage,
      marker,
      milestones: [...state.milestones],
      errorToastSeen: state.errorToastSeen,
      error,
    };
    state.observer.disconnect();
    delete window[stateKey];
    return { kind: "terminal", result };
  };
  const timedOut = () => state.deadline !== null && performance.now() >= state.deadline;

  // ---- task 016: handoff-activation contracts (RFC-042 §6.2 rule 3) ------

  /** Seeded content lengths (shell_behaviour.rs `prepare`), so a focused
   * editor can be told apart from the one that was already open. */
  const docLengths = { child: "# child\n".length, a: "# a\n".length, untitled: 0 };
  const describeActiveElement = () => {
    const active = document.activeElement;
    if (!active) return "none";
    if (active === document.body) return "body";
    const rowIndex = rows().indexOf(active);
    if (rowIndex >= 0) return `tree row ${rowIndex}`;
    if (active.id) return `#${active.id}`;
    const launch = active.getAttribute?.("data-source-focus-launch");
    if (launch) return `[data-source-focus-launch=${launch}]`;
    return String(active.tagName ?? "element").toLowerCase();
  };
  const editorSummary = () => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    return (
      `view=${Boolean(view)} dom.isConnected=${view?.dom?.isConnected} hasFocus=${view?.hasFocus} ` +
      `docLength=${view?.state?.doc?.length} host=${Boolean(host)} ` +
      `statusMarker=${Boolean(host?.querySelector(".source-editor-status"))} ` +
      `activeElement=${describeActiveElement()} toastSeen=${state.errorToastSeen}`
    );
  };
  const editorFocusedWithDoc = (docLength) => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    return Boolean(
      view &&
        view.dom?.isConnected &&
        view.hasFocus &&
        host &&
        !host.querySelector(".source-editor-status") &&
        view.state?.doc?.length === docLength,
    );
  };
  // ---- slice 2: overflow menus (RFC-042 §7.2, RFC-044 §8 B) ------------

  const MENUS = {
    app: { name: "app_menu", trigger: "#app-menu-trigger", menu: "#app-overflow-menu" },
    tools: {
      name: "tools_menu",
      trigger: "#editor-tools-trigger",
      menu: "#editor-tools-menu",
    },
  };
  /** How long a "must not happen" check keeps looking after the positive
   * condition it waited for. An absence cannot be proven by waiting for
   * something to become true: a focus restore scheduled on a later frame
   * (shell_focus::focus_element) can land after the menu's removal has
   * already been observed. This bounds the harness's observation; it is not
   * an app timer (RFC-042 §6.4 concerns focus authority). */
  const ABSENCE_OBSERVATION_FRAMES = 30;
  const menuTrigger = (spec) => document.querySelector(spec.trigger);
  const menuItems = (spec) => {
    const menu = document.querySelector(spec.menu);
    return menu ? [...menu.querySelectorAll('[role="menuitem"]')] : [];
  };
  const describeMenu = (spec, dispatchedTo = null) => {
    const current = menuTrigger(spec);
    // `dispatchedTo` distinguishes "the app ignored the key" from "the key
    // went to a node Dioxus had already replaced": a re-render swaps the
    // button for an identical one, and the stale reference is detached.
    const dispatch = dispatchedTo
      ? ` dispatchedToConnected=${dispatchedTo.isConnected} dispatchedToIsCurrent=${dispatchedTo === current}`
      : "";
    return (
      `menu=${Boolean(document.querySelector(spec.menu))} ` +
      `expanded=${current?.getAttribute("aria-expanded")} ` +
      `items=${menuItems(spec).length} activeElement=${describeActiveElement()}${dispatch}`
    );
  };
  const expectExpanded = (spec, expected, after) => {
    const actual = menuTrigger(spec)?.getAttribute("aria-expanded");
    if (actual !== expected) {
      throw new Error(
        `${spec.name}: aria-expanded is ${actual}, expected ${expected}, after ${after} (${describeMenu(spec)})`,
      );
    }
  };
  const focusMenuTrigger = async (spec) => {
    const trigger = menuTrigger(spec);
    if (!trigger) throw new Error(`${spec.name}: no ${spec.trigger}`);
    trigger.focus();
    const atCall = describeMenu(spec);
    await waitFor(
      () => document.activeElement === trigger,
      () =>
        `${spec.name}: focus onto its own trigger (at call: ${atCall}; at timeout: ${describeMenu(spec)})`,
    );
    return trigger;
  };
  const atMenuEdge = (spec, which) => {
    const items = menuItems(spec);
    const target = which === "first" ? items[0] : items[items.length - 1];
    return items.length > 0 && document.activeElement === target;
  };
  /** Opens by a trigger key and waits for the named item to hold focus.
   * §5: Enter and Space are dispatched only while the trigger is focused --
   * on a menu item they would activate it natively, and "Open Folder" and
   * "Export HTML" open a portal dialog that escapes xvfb (RFC-042 §10). */
  const openMenuWith = async (spec, key, which) => {
    const trigger = await focusMenuTrigger(spec);
    if (document.activeElement !== trigger) {
      throw new Error(
        `${spec.name}: refusing to press ${key}; activeElement is ${describeActiveElement()}, not the trigger`,
      );
    }
    // Re-query: focusing can re-render the bar, and a stale node receives
    // events no handler is attached to any more.
    const target = menuTrigger(spec) ?? trigger;
    dispatchKey(target, key);
    const atDispatch = describeMenu(spec, target);
    await waitFor(
      () => atMenuEdge(spec, which),
      () =>
        `${spec.name}: ${key} on the trigger to open and focus the ${which} item ` +
        `(at dispatch: ${atDispatch}; at timeout: ${describeMenu(spec, target)})`,
    );
    expectExpanded(spec, "true", `${key} on the trigger`);
  };
  const moveWithinMenu = async (spec, key, which) => {
    dispatchKey(document.activeElement, key);
    const atDispatch = describeMenu(spec);
    await waitFor(
      () => atMenuEdge(spec, which),
      () =>
        `${spec.name}: ${key} to reach the ${which} item ` +
        `(at dispatch: ${atDispatch}; at timeout: ${describeMenu(spec)})`,
    );
  };
  /** Contract 6: Escape closes and restores focus to the trigger. */
  const escapeMenu = async (spec) => {
    dispatchKey(document.activeElement ?? menuTrigger(spec), "Escape");
    const atDispatch = describeMenu(spec);
    await waitFor(
      () => !document.querySelector(spec.menu) && document.activeElement === menuTrigger(spec),
      () =>
        `${spec.name}: Escape to close and restore focus to the trigger ` +
        `(at dispatch: ${atDispatch}; at timeout: ${describeMenu(spec)})`,
    );
    expectExpanded(spec, "false", "Escape");
  };
  /** Contract 7, per handoff §4: a synthetic Tab moves nothing, so this moves
   * focus to an element outside the wrap -- script focus() does fire focusin,
   * which is bekoedit's half of the close. Shared with slice 3 (§8 D). */
  const focusLeavesMenu = async (spec, outside, label, moveFocus = () => outside.focus()) => {
    moveFocus();
    const atFocus = describeMenu(spec);
    await waitFor(
      () => !document.querySelector(spec.menu) && document.activeElement === outside,
      () =>
        `${spec.name}: focus leaving to ${label} to close the menu ` +
        `(at focus: ${atFocus}; at timeout: ${describeMenu(spec)})`,
    );
    expectExpanded(spec, "false", "focus leaving the menu");
    // The close was observed; a restore may still be one frame behind it.
    for (let frame = 0; frame <= ABSENCE_OBSERVATION_FRAMES; frame += 1) {
      if (document.activeElement === menuTrigger(spec)) {
        throw new Error(
          `${spec.name}: focus was restored to the trigger; implicit dismissal must not restore (${describeMenu(spec)})`,
        );
      }
      if (frame < ABSENCE_OBSERVATION_FRAMES) {
        await new Promise((resolve) => requestAnimationFrame(resolve));
      }
    }
    if (document.activeElement !== outside) {
      throw new Error(
        `${spec.name}: focus did not stay on ${label} after the menu closed (${describeMenu(spec)})`,
      );
    }
  };
  /** Task 017 §2's mouse rows: a mouse open leaves focus on the trigger. A
   * keyboard entry intent left behind would move it into the menu instead. */
  const mouseOpenLeavesFocus = async (spec, label) => {
    await focusMenuTrigger(spec);
    menuTrigger(spec).click();
    await waitFor(
      () => Boolean(document.querySelector(spec.menu)),
      () => `${spec.name}: a mouse open (${label}) to show the menu (at timeout: ${describeMenu(spec)})`,
    );
    // Long enough for the container's onmounted entry and its frame to run.
    for (let frame = 0; frame < ABSENCE_OBSERVATION_FRAMES; frame += 1) {
      if (menuItems(spec).includes(document.activeElement)) {
        throw new Error(
          `${spec.name}: a mouse open (${label}) moved focus into the menu (${describeMenu(spec)})`,
        );
      }
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    if (document.activeElement !== menuTrigger(spec)) {
      throw new Error(
        `${spec.name}: a mouse open (${label}) left focus on ${describeActiveElement()}, not the trigger`,
      );
    }
    await escapeMenu(spec);
  };
  /** Watches `check` for ABSENCE_OBSERVATION_FRAMES frames (and once more),
   * throwing the first violation it returns. For every "must not happen"
   * assertion after the positive condition it follows (slice 3 handoff §5). */
  const observeWindow = async (check) => {
    for (let frame = 0; frame <= ABSENCE_OBSERVATION_FRAMES; frame += 1) {
      const violation = check();
      if (violation) throw new Error(violation);
      if (frame < ABSENCE_OBSERVATION_FRAMES) {
        await new Promise((resolve) => requestAnimationFrame(resolve));
      }
    }
  };

  // ---- slice 3 stage 1: mode tabs (§8 C) and focus authority (§8 D) -----

  const modeTabs = () => {
    const list = document.querySelector("#editor-mode-switch");
    return list ? [...list.querySelectorAll('[role="tab"]')] : [];
  };
  /** By launch id, exactly one match -- as task 016's Form tab. */
  const uniqueTab = (launchId) => {
    const matches = [...document.querySelectorAll(`[data-source-focus-launch="${launchId}"]`)];
    if (matches.length !== 1) {
      throw new Error(
        `expected exactly one [data-source-focus-launch="${launchId}"], found ${matches.length}`,
      );
    }
    return matches[0];
  };
  const describeTabs = () =>
    `tabs=[${modeTabs()
      .map(
        (tab) =>
          `${tab.getAttribute("data-source-focus-launch")}:selected=${tab.getAttribute(
            "aria-selected",
          )}:tabindex=${tab.getAttribute("tabindex")}`,
      )
      .join(" ")}] activeElement=${describeActiveElement()}`;
  const selectionViolation = (launchId) => {
    const selected = modeTabs().filter((tab) => tab.getAttribute("aria-selected") === "true");
    if (selected.length !== 1) return `expected exactly one selected tab, found ${selected.length}`;
    const id = selected[0].getAttribute("data-source-focus-launch");
    if (id !== launchId) return `the selected tab is ${id}, expected ${launchId}`;
    if (selected[0].getAttribute("tabindex") !== "0") {
      return `the selected tab ${id} does not hold tabindex=0`;
    }
    return null;
  };
  const focusTab = async (launchId, contract) => {
    const tab = uniqueTab(launchId);
    tab.focus();
    await waitFor(
      () => document.activeElement === tab,
      () => `${contract}: focus onto the ${launchId} tab (at timeout: ${describeTabs()})`,
    );
    return tab;
  };
  /** Activation through bekoedit's half (handoff §3.3): a script click runs
   * the tab's onclick, which a real Enter or Space reaches only through the
   * browser's native activation. The tab is focused first, as it is for a
   * keyboard user pressing Enter on it: the source focus guard accepts a
   * persistent control's claim only while its launch origin holds focus
   * (launchMustRemain), and a script click moves no focus. The editor focus
   * asserted afterwards still comes from the claim, not from the click. */
  const activateTab = async (launchId, contract) => {
    const tab = await focusTab(launchId, contract);
    tab.click();
    return tab;
  };
  // ---- slice 3 stage 2: Settings and Recovery (§8 E), conflict banner (§8 F)

  const recoveryRegion = () =>
    document.querySelector('[role="region"][aria-labelledby="recovery-heading"]');
  const settingsRegion = () =>
    document.querySelector('[role="region"][aria-labelledby="settings-heading"]');
  const conflictBanner = () => document.querySelector('.conflict-banner[role="alert"]');
  const activeId = () => document.activeElement?.id ?? null;
  const describeScreens = () =>
    `recovery=${Boolean(recoveryRegion())} settings=${Boolean(settingsRegion())} ` +
    `treeRows=${rows().length} activeElement=${describeActiveElement()}`;
  const describeConflict = () => {
    const banner = conflictBanner();
    return (
      `banner=${Boolean(banner)} buttons=${banner ? banner.querySelectorAll("button").length : 0} ` +
      `dirtyDot=${Boolean(document.querySelector(".dirty-dot"))} ` +
      `fileName=${document.querySelector(".file-name")?.textContent?.trim() ?? null} ` +
      `activeElement=${describeActiveElement()}`
    );
  };
  const editorMounted = () => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    return Boolean(
      view && view.dom?.isConnected && host && !host.querySelector(".source-editor-status"),
    );
  };
  /** A screen or restore can take longer than a nav key: it waits on a
   * render, and on a remounted editor. */
  const SCREEN_TIMEOUT_MS = 10000;
  const rovingTreeRow = () => {
    const row = rows().find((candidate) => candidate.getAttribute("tabindex") === "0");
    if (!row) throw new Error("no tree row at tabindex=0 to move focus out of the menu");
    return row;
  };
  const MENU_PHASES = {
    app_menu_mouse_open: {
      spec: MENUS.app,
      kind: "mouseOpen",
      milestone: "app_menu_mouse_open_kept_focus",
      next: "app_menu_keys",
    },
    app_menu_keys: {
      spec: MENUS.app,
      kind: "keys",
      milestone: "app_menu_keys_verified",
      next: "app_menu_escape",
    },
    app_menu_escape: {
      spec: MENUS.app,
      kind: "escape",
      milestone: "app_menu_escape_restored",
      next: "app_menu_focus_leave",
    },
    app_menu_focus_leave: {
      spec: MENUS.app,
      kind: "focusLeave",
      milestone: "app_menu_focus_leave_kept",
      next: "tools_menu_keys",
    },
    tools_menu_keys: {
      spec: MENUS.tools,
      kind: "keys",
      milestone: "tools_menu_keys_verified",
      next: "tools_menu_escape",
    },
    tools_menu_escape: {
      spec: MENUS.tools,
      kind: "escape",
      milestone: "tools_menu_escape_restored",
      next: "tools_menu_focus_leave",
    },
    tools_menu_focus_leave: {
      spec: MENUS.tools,
      kind: "focusLeave",
      milestone: "tools_menu_focus_leave_kept",
      next: "tabs_arrows_focus_only",
    },
  };

  /** Leaves the current phase for `next` with a fresh step and deadline, so
   * no phase ever reads the previous one's deadline. */
  const advance = (milestone, next) => {
    if (state.errorToastSeen) throw new Error("an error toast appeared");
    state.milestones.push(milestone);
    state.phase = next;
    state.step = 0;
    state.deadline = null;
    return { kind: "progress", milestone };
  };

  let outgoing;
  try {
    if (state.protocolVersion !== 1 || requestedPhase !== state.phase) {
      throw new Error(
        `phase mismatch: requested ${requestedPhase}, current ${state.phase}`,
      );
    }

    // Row 0 is always the workspace root itself, already expanded --
    // explorer.rs auto-expands it on mount (collect_rows in
    // dioxus-swdir-tree-core pushes the root as a row before recursing
    // into its children). So the four seeded files are rows 1-4: at
    // least 5 rows total once the root's own scan has merged.
    // Named before the preamble, so a preamble failure names the phase that
    // hit it rather than the one before (each branch still sets its own).
    state.stage = requestedPhase;
    // Phases that start while a screen replacement covers MainShell, so no
    // tree renders: Recovery at launch (slice 3 §4.1), and Settings between
    // E3 and E4.
    if (!SHELL_REPLACED_AT_START.has(requestedPhase)) {
      await waitFor(
        () => rows().length >= 5,
        "the workspace tree to render its rows (root + four seeded entries)",
      );
    }

    if (requestedPhase === "down_up") {
      // Root(0) -> sub(1) -> a.md(2) -> sub(1) -> root(0): still a full
      // exercise of contract 2 (Down/Up move the active row) and contract
      // 1's invariant at each step: it does not matter that row 0 happens
      // to be the root rather than a seeded file.
      state.stage = "down_up";
      rows()[0].focus();
      if (document.activeElement !== rows()[0]) {
        throw new Error("could not focus the first tree row directly");
      }
      dispatchKey(rows()[0], "ArrowDown");
      await waitFor(() => document.activeElement === rows()[1], "ArrowDown to reach the second row");
      await checkTabStopInvariant(1);
      dispatchKey(rows()[1], "ArrowDown");
      await waitFor(() => document.activeElement === rows()[2], "ArrowDown to reach the third row");
      await checkTabStopInvariant(2);
      dispatchKey(rows()[2], "ArrowUp");
      await waitFor(() => document.activeElement === rows()[1], "ArrowUp to return to the second row");
      await checkTabStopInvariant(1);
      dispatchKey(rows()[1], "ArrowUp");
      await waitFor(() => document.activeElement === rows()[0], "ArrowUp to return to the first row");
      await checkTabStopInvariant(0);
      state.milestones.push("down_up_moved");
      state.phase = "expand_enter";
      outgoing = { kind: "progress", milestone: "down_up_moved" };
    } else if (requestedPhase === "expand_enter") {
      // sub (row 1, the first seeded entry) is genuinely unloaded, so
      // expanding it triggers a real, async filesystem scan through the
      // tree's own coroutine (on_toggled -> ScanRequest -> background
      // thread -> on_loaded merge) -- not a synchronous state flip, so
      // this phase is multi-call and pollable (like enter_opens), not a
      // single blocking wait: a document::eval call has its own outer
      // deadline (the Rust-side evaluator timeout) shorter than the scan
      // can be relied on to finish within.
      state.stage = "expand_enter";
      if (timedOut()) {
        throw new Error(
          `timed out at expand_enter: row count is ${rows().length} (expected ${
            (state.expandBeforeCount ?? "?") + 1
          }), row 1 aria-expanded=${rows()[1]?.getAttribute("aria-expanded")}, ` +
            `activeElement is row ${rows().findIndex((row) => row === document.activeElement)}`,
        );
      }
      if (!state.expandMoved) {
        dispatchKey(rows()[0], "ArrowDown");
        state.expandMoved = true;
        state.deadline = performance.now() + 10000;
        outgoing = { kind: "pending" };
      } else if (!state.expandDispatched) {
        if (document.activeElement !== rows()[1]) {
          outgoing = { kind: "pending" };
        } else {
          await checkTabStopInvariant(1);
          state.expandBeforeCount = rows().length;
          dispatchKey(rows()[1], "ArrowRight");
          state.expandDispatched = true;
          outgoing = { kind: "pending" };
        }
      } else if (!state.expandConfirmed) {
        if (rows().length !== state.expandBeforeCount + 1) {
          outgoing = { kind: "pending" };
        } else {
          if (rows()[1].getAttribute("aria-expanded") !== "true") {
            throw new Error("expanded row did not report aria-expanded=true");
          }
          if (document.activeElement !== rows()[1]) {
            throw new Error("expanding must not move focus off the directory row");
          }
          await checkTabStopInvariant(1);
          state.expandConfirmed = true;
          dispatchKey(rows()[1], "ArrowRight");
          outgoing = { kind: "pending" };
        }
      } else if (document.activeElement !== rows()[2]) {
        outgoing = { kind: "pending" };
      } else {
        await checkTabStopInvariant(2);
        state.milestones.push("expand_entered");
        state.phase = "collapse_ascend";
        outgoing = { kind: "progress", milestone: "expand_entered" };
      }
    } else if (requestedPhase === "collapse_ascend") {
      // Collapse is always synchronous (on_toggled's Case B: the
      // generation is not bumped, no scan involved) -- a single blocking
      // wait is fine here, unlike the first expand.
      state.stage = "collapse_ascend";
      dispatchKey(rows()[2], "ArrowLeft");
      await waitFor(
        () => document.activeElement === rows()[1],
        "ArrowLeft to ascend from the child row to its parent directory",
      );
      await checkTabStopInvariant(1);
      const before = rows().length;
      dispatchKey(rows()[1], "ArrowLeft");
      await waitFor(
        () => rows().length === before - 1,
        "ArrowLeft to collapse the expanded directory (one fewer row)",
      );
      if (rows()[1].getAttribute("aria-expanded") !== "false") {
        throw new Error("collapsed row did not report aria-expanded=false");
      }
      if (document.activeElement !== rows()[1]) {
        throw new Error("collapsing must not move focus off the directory row");
      }
      await checkTabStopInvariant(1);
      state.milestones.push("collapse_ascended");
      state.phase = "home_end";
      outgoing = { kind: "progress", milestone: "collapse_ascended" };
    } else if (requestedPhase === "home_end") {
      state.stage = "home_end";
      const last = rows().length - 1;
      dispatchKey(rows()[1], "End");
      await waitFor(() => document.activeElement === rows()[last], "End to reach the last row");
      await checkTabStopInvariant(last);
      dispatchKey(rows()[last], "Home");
      await waitFor(() => document.activeElement === rows()[0], "Home to reach the first row");
      await checkTabStopInvariant(0);
      state.milestones.push("home_end_reached");
      state.phase = "non_openable";
      outgoing = { kind: "progress", milestone: "home_end_reached" };
    } else if (requestedPhase === "non_openable") {
      state.stage = "non_openable";
      dispatchKey(rows()[0], "ArrowDown");
      await waitFor(() => document.activeElement === rows()[1], "ArrowDown to reach the second row");
      dispatchKey(rows()[1], "ArrowDown");
      await waitFor(() => document.activeElement === rows()[2], "ArrowDown to reach the third row");
      dispatchKey(rows()[2], "ArrowDown");
      await waitFor(
        () => document.activeElement === rows()[3],
        "ArrowDown to reach the non-openable row (it must not be skipped)",
      );
      await checkTabStopInvariant(3);
      const target = rows()[3];
      if (target.getAttribute("aria-disabled") !== "true") {
        throw new Error("the reached row is not actually non-openable (aria-disabled != true)");
      }
      dispatchKey(target, "Enter");
      // Enter is app-intercepted (prevent_default runs) but a non-openable
      // row's activation is a no-op -- nothing to poll for, so this is the
      // one nav step checked immediately rather than via waitFor.
      if (document.activeElement !== target) {
        throw new Error("Enter on a non-openable row unexpectedly moved focus");
      }
      if (document.querySelector(".toast-error")) {
        throw new Error("Enter on a non-openable row unexpectedly raised an error toast");
      }
      state.milestones.push("non_openable_reachable");
      state.phase = "enter_opens";
      outgoing = { kind: "progress", milestone: "non_openable_reachable" };
    } else if (requestedPhase === "enter_opens") {
      // Contract 7 (RFC-044 §8 A, task 014): Enter on a Markdown row opens it
      // and the editor takes focus. Multi-call and pollable -- opening a
      // document mounts a fresh source editor, which is real async work.
      state.stage = "enter_opens";
      // Only this phase's own deadline counts: state.deadline still holds
      // expand_enter's until Enter is dispatched below.
      if (state.enterDispatched && timedOut()) {
        const view = window.__bk?._view;
        const host = document.querySelector('[data-source-focus-launch-region="text"]');
        const activeRowIndex = rows().findIndex((row) => row === document.activeElement);
        const activeRow = rows()[activeRowIndex];
        throw new Error(
          `timed out at enter_opens: view=${Boolean(view)} dom.isConnected=${view?.dom?.isConnected} ` +
            `hasFocus=${view?.hasFocus} host=${Boolean(host)} statusMarker=${Boolean(
              host?.querySelector(".source-editor-status"),
            )} activeElement is row ${activeRowIndex} (title=${activeRow?.getAttribute?.(
              "title",
            )}) toastSeen=${state.errorToastSeen}`,
        );
      }
      if (!state.enterDispatched) {
        dispatchKey(rows()[3], "ArrowUp");
        await waitFor(
          () => document.activeElement === rows()[2],
          "ArrowUp to return to the markdown row",
        );
        await checkTabStopInvariant(2);
        dispatchKey(rows()[2], "Enter");
        state.enterDispatched = true;
        state.deadline = performance.now() + 15000;
        outgoing = { kind: "pending" };
      } else {
        const view = window.__bk?._view;
        const host = document.querySelector('[data-source-focus-launch-region="text"]');
        const ready =
          view &&
          view.dom?.isConnected &&
          view.hasFocus &&
          host &&
          !host.querySelector(".source-editor-status");
        if (!ready) {
          outgoing = { kind: "pending" };
        } else {
          outgoing = advance("enter_opened_editor_focused", "search_result_opens");
        }
      }
    } else if (requestedPhase === "search_result_opens") {
      // Task 016 §5.2 (a): activating a search result opens its document and
      // the editor takes focus -- not the search trigger. Multi-call: the
      // search itself runs in a spawned task.
      state.stage = "search_result_opens";
      const trigger = document.querySelector("#workspace-search-trigger");
      if (state.step > 0 && timedOut()) {
        throw new Error(
          `timed out at search_result_opens (step ${state.step}): ` +
            `results=${document.querySelectorAll(".search-match-btn").length} ${editorSummary()}`,
        );
      }
      if (state.step === 0) {
        if (!trigger) throw new Error("search_result_opens: no #workspace-search-trigger");
        trigger.click();
        await waitFor(() => document.querySelector("#workspace-search-input"), "the search panel to open");
        const input = document.querySelector("#workspace-search-input");
        input.value = "child";
        input.dispatchEvent(new Event("input", { bubbles: true }));
        state.step = 1;
        state.deadline = performance.now() + 15000;
        outgoing = { kind: "pending" };
      } else if (state.step === 1) {
        const input = document.querySelector("#workspace-search-input");
        if (!input) throw new Error("search_result_opens: the search input disappeared");
        dispatchKey(input, "Enter");
        state.step = 2;
        outgoing = { kind: "pending" };
      } else if (state.step === 2) {
        const result = [...document.querySelectorAll(".search-match-btn")].find((button) =>
          button.textContent.includes("child.md"),
        );
        if (result) {
          result.click();
          state.step = 3;
        }
        outgoing = { kind: "pending" };
      } else if (editorFocusedWithDoc(docLengths.child) && document.activeElement !== trigger) {
        outgoing = advance("search_result_editor_focused", "new_file_focuses");
      } else {
        outgoing = { kind: "pending" };
      }
    } else if (requestedPhase === "new_file_focuses") {
      // Task 016 §5.2 (b), first assertion: App menu "New File" focuses the
      // editor.
      state.stage = "new_file_focuses";
      if (state.step > 0 && timedOut()) {
        throw new Error(`timed out at new_file_focuses: ${editorSummary()}`);
      }
      if (state.step === 0) {
        const trigger = document.querySelector("#app-menu-trigger");
        if (!trigger) throw new Error("new_file_focuses: no #app-menu-trigger");
        trigger.click();
        await waitFor(() => document.querySelector("#app-overflow-menu"), "the app menu to open");
        // §5.3: by launch id only, never by position. "Open Folder" sits
        // beside it and opens a native dialog that escapes xvfb.
        const launches = [...document.querySelectorAll('[data-source-focus-launch="appbar-new"]')];
        if (launches.length !== 1) {
          throw new Error(
            `new_file_focuses: expected exactly one [data-source-focus-launch="appbar-new"], found ${launches.length}`,
          );
        }
        const item = launches[0];
        if (
          item.getAttribute("data-source-focus-launch") !== "appbar-new" ||
          item.getAttribute("role") !== "menuitem"
        ) {
          throw new Error("new_file_focuses: the appbar-new launch is not a menu item");
        }
        item.click();
        state.step = 1;
        state.deadline = performance.now() + 15000;
        outgoing = { kind: "pending" };
      } else if (editorFocusedWithDoc(docLengths.untitled)) {
        outgoing = advance("new_file_editor_focused", "tree_enter_after_new_file");
      } else {
        outgoing = { kind: "pending" };
      }
    } else if (requestedPhase === "tree_enter_after_new_file") {
      // Task 016 §5.2 (b), second assertion: a tree Enter after "New File"
      // still focuses the editor. While the menu's shell authority stays
      // held, task 014's claim is refused and focus stays on the row.
      state.stage = "tree_enter_after_new_file";
      if (state.step > 0 && timedOut()) {
        throw new Error(`timed out at tree_enter_after_new_file: ${editorSummary()}`);
      }
      if (state.step === 0) {
        const row = rows()[2];
        if (row?.getAttribute("aria-disabled") !== "false") {
          throw new Error("tree_enter_after_new_file: row 2 is not the openable a.md row");
        }
        row.focus();
        await waitFor(() => document.activeElement === row, "focus onto the a.md row");
        dispatchKey(row, "Enter");
        state.step = 1;
        state.deadline = performance.now() + 15000;
        outgoing = { kind: "pending" };
      } else if (editorFocusedWithDoc(docLengths.a)) {
        outgoing = advance("tree_enter_refocused_after_new_file", "form_search_restores");
      } else {
        outgoing = { kind: "pending" };
      }
    } else if (requestedPhase === "form_search_restores") {
      // Task 016 re-review §2: in Form -- the default mode -- a search result
      // claims no editor focus, so it is not a handoff. Explicit dismissal
      // applies: the document opens and focus returns to the search trigger.
      state.stage = "form_search_restores";
      const trigger = document.querySelector("#workspace-search-trigger");
      const formTabs = () => [...document.querySelectorAll('[data-source-focus-launch="mode-form"]')];
      const fileName = () => document.querySelector(".file-name")?.textContent?.trim() ?? null;
      if (state.step > 0 && timedOut()) {
        throw new Error(
          `timed out at form_search_restores (step ${state.step}): ` +
            `formSelected=${formTabs()[0]?.getAttribute("aria-selected")} fileName=${fileName()} ` +
            `results=${document.querySelectorAll(".search-match-btn").length} ` +
            `activeElement=${describeActiveElement()} toastSeen=${state.errorToastSeen}`,
        );
      }
      if (state.step === 0) {
        // By launch id only, never by position (same rule as appbar-new).
        const tabs = formTabs();
        if (tabs.length !== 1) {
          throw new Error(
            `form_search_restores: expected exactly one [data-source-focus-launch="mode-form"], found ${tabs.length}`,
          );
        }
        tabs[0].click();
        state.step = 1;
        state.deadline = performance.now() + 15000;
        outgoing = { kind: "pending" };
      } else if (state.step === 1) {
        if (formTabs()[0]?.getAttribute("aria-selected") === "true") {
          if (!trigger) throw new Error("form_search_restores: no #workspace-search-trigger");
          trigger.click();
          await waitFor(() => document.querySelector("#workspace-search-input"), "the search panel to open");
          const input = document.querySelector("#workspace-search-input");
          input.value = "child";
          input.dispatchEvent(new Event("input", { bubbles: true }));
          state.step = 2;
        }
        outgoing = { kind: "pending" };
      } else if (state.step === 2) {
        const input = document.querySelector("#workspace-search-input");
        if (!input) throw new Error("form_search_restores: the search input disappeared");
        dispatchKey(input, "Enter");
        state.step = 3;
        outgoing = { kind: "pending" };
      } else if (state.step === 3) {
        const result = [...document.querySelectorAll(".search-match-btn")].find((button) =>
          button.textContent.includes("child.md"),
        );
        if (result) {
          result.click();
          state.step = 4;
        }
        outgoing = { kind: "pending" };
      } else if (fileName() === "child.md" && trigger && document.activeElement === trigger) {
        outgoing = advance("form_search_restored_to_trigger", "app_menu_mouse_open");
      } else {
        outgoing = { kind: "pending" };
      }
    } else if (requestedPhase === "recovery_entry") {
      // §8 E1: the seeded snapshot's Recovery screen, focus on its heading.
      state.stage = "recovery_entry";
      await waitFor(
        () => Boolean(recoveryRegion()) && activeId() === "recovery-heading",
        () => `E1: the Recovery screen with focus on #recovery-heading (at timeout: ${describeScreens()})`,
        SCREEN_TIMEOUT_MS,
      );
      const status = recoveryRegion().querySelector('[role="status"]');
      const text = status?.textContent?.trim() ?? null;
      if (!text?.startsWith("1 ")) {
        throw new Error(`E1: the Recovery status does not report one snapshot: "${text}" (${describeScreens()})`);
      }
      outgoing = advance("recovery_heading_focused", "recovery_exit");
    } else if (requestedPhase === "recovery_exit") {
      // §8 E2: Skip all -- never Restore or Discard (§6.2).
      state.stage = "recovery_exit";
      const skips = [...document.querySelectorAll(".recovery-skip")];
      if (skips.length !== 1) {
        throw new Error(`E2: refusing to click: expected exactly one .recovery-skip, found ${skips.length} (${describeScreens()})`);
      }
      const atClick = describeScreens();
      skips[0].click();
      await waitFor(
        () => !recoveryRegion() && rows().length >= 5 && activeId() === "app-bar-logo-trigger",
        () =>
          `E2: Skip all to close Recovery and restore focus to #app-bar-logo-trigger ` +
          `(at click: ${atClick}; at timeout: ${describeScreens()})`,
        SCREEN_TIMEOUT_MS,
      );
      await observeWindow(() =>
        activeId() === "app-bar-logo-trigger"
          ? null
          : `E2: focus did not stay on #app-bar-logo-trigger after Recovery closed (${describeScreens()})`,
      );
      outgoing = advance("recovery_exit_restored_logo", "down_up");
    } else if (requestedPhase === "settings_entry") {
      // §8 E3: open the app menu by keyboard, then click Settings by its id,
      // behind §6.1's guard -- its neighbours open native dialogs.
      state.stage = "settings_entry";
      await openMenuWith(MENUS.app, "ArrowDown", "first");
      const menu = document.querySelector(MENUS.app.menu);
      const found = [...document.querySelectorAll("#app-menu-settings")];
      const inside = Boolean(menu && found[0] && menu.contains(found[0]));
      if (found.length !== 1 || !inside || found[0].id !== "app-menu-settings") {
        throw new Error(
          `E3: refusing to click: expected exactly one #app-menu-settings inside ${MENUS.app.menu}, ` +
            `found ${found.length} (inside the menu: ${inside}) (${describeMenu(MENUS.app)})`,
        );
      }
      found[0].click();
      await waitFor(
        () => Boolean(settingsRegion()) && activeId() === "settings-heading",
        () => `E3: Settings with focus on #settings-heading (at timeout: ${describeScreens()})`,
        SCREEN_TIMEOUT_MS,
      );
      outgoing = advance("settings_heading_focused", "settings_exit_restored");
    } else if (requestedPhase === "settings_exit_restored") {
      // §8 E4: Close -- never Save (§6.2). The window starts once the
      // Text-mode editor has remounted, since a claim or focus it made would
      // land after the restore (handoff §2's correction).
      state.stage = "settings_exit_restored";
      const region = settingsRegion();
      const closes = [...document.querySelectorAll("#settings-close")];
      if (closes.length !== 1 || !region?.contains(closes[0])) {
        throw new Error(
          `E4: refusing to click: expected exactly one #settings-close inside Settings, found ${closes.length} (${describeScreens()})`,
        );
      }
      const atClick = describeScreens();
      closes[0].click();
      await waitFor(
        () => !settingsRegion() && editorMounted() && activeId() === "app-menu-trigger",
        () =>
          `E4: Close to leave Settings, remount the editor and restore focus to #app-menu-trigger ` +
          `(at click: ${atClick}; at timeout: ${describeScreens()} ${editorSummary()})`,
        SCREEN_TIMEOUT_MS,
      );
      await observeWindow(() =>
        activeId() === "app-menu-trigger"
          ? null
          : `E4: focus did not stay on #app-menu-trigger after Settings closed (${describeScreens()} ${editorSummary()})`,
      );
      outgoing = advance("settings_exit_restored_trigger", "conflict_dirtied");
    } else if (requestedPhase === "conflict_dirtied") {
      // §8 F1: one unsaved insertion, as RFC-041's edit_dispatched does. No
      // Enter, Space or click anywhere in F (§6.2).
      state.stage = "conflict_dirtied";
      const fileName = document.querySelector(".file-name")?.textContent?.trim() ?? null;
      if (fileName !== "child.md") {
        throw new Error(`F1: the open document is ${fileName}, not child.md; the Rust write would miss it (${describeConflict()})`);
      }
      const view = window.__bk?._view;
      if (!view) throw new Error(`F1: no editor view to edit (${describeConflict()})`);
      view.focus();
      await waitFor(
        () => view.hasFocus,
        () => `F1: focus into the editor before the edit (at timeout: ${describeConflict()})`,
      );
      view.dispatch({ changes: { from: view.state.doc.length, insert: "\nAn unsaved edit (RFC-044 §8 F).\n" } });
      await waitFor(
        () => Boolean(document.querySelector(".dirty-dot")),
        () => `F1: the header's dirty dot after the edit (at timeout: ${describeConflict()})`,
        SCREEN_TIMEOUT_MS,
      );
      outgoing = advance("conflict_document_dirtied", "conflict_banner_focus_kept");
    } else if (requestedPhase === "conflict_banner_focus_kept") {
      // §8 F2: the Rust sequence wrote the file between F1 and this phase.
      // The banner appears and announces; focus does not move (RFC-042 §7.6).
      state.stage = "conflict_banner_focus_kept";
      const view = window.__bk?._view;
      const recorded = document.activeElement;
      const inEditor =
        Boolean(view?.contentDOM) && (recorded === view.contentDOM || view.contentDOM.contains?.(recorded));
      if (!inEditor) {
        throw new Error(`F2: focus is not inside the editor before the banner: ${describeActiveElement()} (${describeConflict()})`);
      }
      await waitFor(
        () => Boolean(conflictBanner()),
        () => `F2: the conflict banner after the on-disk change (at timeout: ${describeConflict()})`,
        SCREEN_TIMEOUT_MS,
      );
      const buttons = conflictBanner().querySelectorAll("button").length;
      if (buttons !== 3) {
        throw new Error(`F2: expected the dirty-memory banner's three actions, found ${buttons} (${describeConflict()})`);
      }
      await observeWindow(() => {
        const banner = conflictBanner();
        const active = document.activeElement;
        if (banner && (banner === active || banner.contains?.(active))) {
          return `F2: focus moved into the conflict banner (${describeConflict()})`;
        }
        if (active !== recorded) {
          return `F2: focus left the editor when the banner appeared (${describeConflict()})`;
        }
        return null;
      });
      if (state.errorToastSeen) throw new Error("an error toast appeared");
      state.milestones.push("conflict_banner_focus_kept");
      outgoing = finish(true);
    } else if (requestedPhase === "tabs_arrows_focus_only") {
      // §8 C1: Right, Left, Home, End move focus and nothing else. Checked
      // after every key -- Right then Left starts and ends on Form, so one
      // check at the end would pass a build that switched mode twice.
      state.stage = "tabs_arrows_focus_only";
      await focusTab("mode-form", "C1");
      const before = selectionViolation("mode-form");
      if (before) throw new Error(`C1: before any key, ${before} (${describeTabs()})`);
      for (const [key, expected] of [
        ["ArrowRight", "mode-text"],
        ["ArrowLeft", "mode-form"],
        ["Home", "mode-text"],
        ["End", "mode-form"],
      ]) {
        const atDispatch = describeTabs();
        dispatchKey(document.activeElement, key);
        await waitFor(
          () => document.activeElement === uniqueTab(expected),
          () =>
            `C1: ${key} to move focus to ${expected} ` +
            `(at dispatch: ${atDispatch}; at timeout: ${describeTabs()})`,
        );
        await observeWindow(() => {
          const violation = selectionViolation("mode-form");
          if (violation) {
            return `C1: ${key} changed the selected tab, not only focus: ${violation} (${describeTabs()})`;
          }
          if (document.activeElement !== uniqueTab(expected)) {
            return `C1: focus did not stay on ${expected} after ${key} (${describeTabs()})`;
          }
          return null;
        });
      }
      outgoing = advance("tabs_arrows_moved_focus_only", "tabs_click_activates");
    } else if (requestedPhase === "tabs_click_activates") {
      // §8 C2: activating Text selects it and SwitchMode(Text) claims the editor.
      state.stage = "tabs_click_activates";
      await activateTab("mode-text", "C2");
      await waitFor(
        () => selectionViolation("mode-text") === null && editorFocusedWithDoc(docLengths.child),
        () =>
          `C2: activating the Text tab to select it and focus the editor ` +
          `(at timeout: ${describeTabs()} ${editorSummary()})`,
      );
      outgoing = advance("tabs_click_focused_editor", "menu_closes_into_editor");
    } else if (requestedPhase === "menu_closes_into_editor") {
      // §8 D1: with the app menu open, focus entering the editor closes it,
      // without a restore to the trigger, and focus stays in the editor.
      state.stage = "menu_closes_into_editor";
      await openMenuWith(MENUS.app, "ArrowDown", "first");
      const view = window.__bk?._view;
      if (!view?.contentDOM) throw new Error(`D1: no editor view to focus (${editorSummary()})`);
      await focusLeavesMenu(MENUS.app, view.contentDOM, "the source editor", () => view.focus());
      if (!view.hasFocus) {
        throw new Error(`D1: view.hasFocus is false after the menu closed (${editorSummary()})`);
      }
      outgoing = advance("menu_closed_into_editor_kept", "authority_released_after_editor_focus");
    } else if (requestedPhase === "authority_released_after_editor_focus") {
      // §8 D2: a close that kept shell authority looks right to D1 and refuses
      // every later claim. Prove the release by making one.
      state.stage = "authority_released_after_editor_focus";
      uniqueTab("mode-preview").click(); // Preview claims nothing, so no focus is needed.
      await waitFor(
        () => selectionViolation("mode-preview") === null,
        () => `D2: clicking the Preview tab to select it (at timeout: ${describeTabs()})`,
      );
      await activateTab("mode-text", "D2");
      await waitFor(
        () => selectionViolation("mode-text") === null && editorFocusedWithDoc(docLengths.child),
        () =>
          `D2: activating Text to claim the editor again; a refused claim means the menu's ` +
          `close into the editor kept shell authority (at timeout: ${describeTabs()} ${editorSummary()})`,
      );
      outgoing = advance("authority_released_editor_refocused", "settings_entry");
    } else if (MENU_PHASES[requestedPhase]) {
      const { spec, kind, milestone, next } = MENU_PHASES[requestedPhase];
      state.stage = requestedPhase;
      if (kind === "mouseOpen") {
        await mouseOpenLeavesFocus(spec, "alone");
        await openMenuWith(spec, "ArrowDown", "first");
        await escapeMenu(spec);
        await mouseOpenLeavesFocus(spec, "after a keyboard open and close");
      } else if (kind === "keys") {
        await openMenuWith(spec, "ArrowDown", "first"); // contract 1
        await escapeMenu(spec);
        await openMenuWith(spec, "ArrowUp", "last"); // contract 2
        await escapeMenu(spec);
        await openMenuWith(spec, "Enter", "first"); // contract 3, Enter
        await escapeMenu(spec);
        await openMenuWith(spec, " ", "first"); // contract 3, Space
        await moveWithinMenu(spec, "End", "last"); // contract 5
        await moveWithinMenu(spec, "ArrowDown", "first"); // contract 4, wrap down
        await moveWithinMenu(spec, "ArrowUp", "last"); // contract 4, wrap up
        await moveWithinMenu(spec, "Home", "first"); // contract 5
        await escapeMenu(spec);
      } else if (kind === "escape") {
        await openMenuWith(spec, "ArrowDown", "first");
        await escapeMenu(spec); // contract 6
      } else {
        await openMenuWith(spec, "ArrowDown", "first");
        await focusLeavesMenu(spec, rovingTreeRow(), "the roving tree row"); // contract 7
      }
      if (state.errorToastSeen) throw new Error("an error toast appeared");
      if (next) {
        outgoing = advance(milestone, next);
      } else {
        state.milestones.push(milestone);
        outgoing = finish(true);
      }
    } else {
      throw new Error(`unknown phase: ${requestedPhase}`);
    }
  } catch (error) {
    outgoing = finish(false, String(error));
  }

  const report = {
    protocolVersion,
    exchangeId,
    phase: requestedPhase,
    releasedExchangeId,
    releasedPhase,
    ...outgoing,
  };
  dioxus.send(report);
  const acknowledgement = await dioxus.recv();
  if (
    acknowledgement?.protocolVersion !== protocolVersion ||
    acknowledgement?.exchangeId !== exchangeId ||
    acknowledgement?.phase !== requestedPhase ||
    acknowledgement?.kind !== report.kind
  ) {
    throw new Error("invalid phase acknowledgement");
  }

  if (pinRegistry.current !== null) {
    throw new Error("smoke evaluator pin was already occupied");
  }
  pinRegistry.current = Object.freeze({
    exchangeId,
    phase: requestedPhase,
    channel: dioxus,
  });
  return {
    protocolVersion,
    exchangeId,
    phase: requestedPhase,
    kind: report.kind,
    acknowledgementProcessed: true,
    evaluatorPinned: true,
  };
})();
