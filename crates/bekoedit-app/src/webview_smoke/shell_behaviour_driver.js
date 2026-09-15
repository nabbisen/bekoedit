return (async () => {
  const marker = "RFC044_SHELL_BEHAVIOUR_MARKER";
  const stateKey = "__bkWebViewShellBehaviourState";
  const pinKey = "__bkWebViewSmokeEvalPin";
  const protocolVersion = 2;
  const pinProtocolVersion = 1;
  const phases = [
    "down_up",
    "expand_enter",
    "collapse_ascend",
    "home_end",
    "non_openable",
    "enter_opens",
    "search_result_opens",
    "new_file_focuses",
    "tree_enter_after_new_file",
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
        throw new Error(`timed out waiting for: ${description}`);
      }
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
  };
  /** RFC-044 §8 A.1's corrected mechanism: a synthetic Tab cannot drive
   * focus (untrusted events get no browser default action), so contract 1
   * is this invariant, asserted live after each app-intercepted nav key
   * instead -- exactly one row at tabindex=0, it is the row that just
   * became active, and it is document.activeElement. */
  const checkTabStopInvariant = (expectedIndex) => {
    const all = rows();
    const zeroed = all.filter((row) => row.getAttribute("tabindex") === "0");
    if (zeroed.length !== 1) {
      throw new Error(
        `roving-tabindex invariant: expected exactly one row at tabindex=0, found ${zeroed.length}`,
      );
    }
    if (all[expectedIndex] !== zeroed[0]) {
      throw new Error(
        "roving-tabindex invariant: the tabindex=0 row is not the row that just became active",
      );
    }
    if (document.activeElement !== zeroed[0]) {
      throw new Error("roving-tabindex invariant: the tabindex=0 row is not document.activeElement");
    }
  };

  const containsErrorToast = (node) =>
    node?.nodeType === Node.ELEMENT_NODE &&
    (node.matches?.(".toast-error") || node.querySelector?.(".toast-error"));

  const createState = () => {
    const state = {
      protocolVersion: 1,
      phase: "down_up",
      stage: "down_up",
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
    await waitFor(
      () => rows().length >= 5,
      "the workspace tree to render its rows (root + four seeded entries)",
    );

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
      checkTabStopInvariant(1);
      dispatchKey(rows()[1], "ArrowDown");
      await waitFor(() => document.activeElement === rows()[2], "ArrowDown to reach the third row");
      checkTabStopInvariant(2);
      dispatchKey(rows()[2], "ArrowUp");
      await waitFor(() => document.activeElement === rows()[1], "ArrowUp to return to the second row");
      checkTabStopInvariant(1);
      dispatchKey(rows()[1], "ArrowUp");
      await waitFor(() => document.activeElement === rows()[0], "ArrowUp to return to the first row");
      checkTabStopInvariant(0);
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
          checkTabStopInvariant(1);
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
          checkTabStopInvariant(1);
          state.expandConfirmed = true;
          dispatchKey(rows()[1], "ArrowRight");
          outgoing = { kind: "pending" };
        }
      } else if (document.activeElement !== rows()[2]) {
        outgoing = { kind: "pending" };
      } else {
        checkTabStopInvariant(2);
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
      checkTabStopInvariant(1);
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
      checkTabStopInvariant(1);
      state.milestones.push("collapse_ascended");
      state.phase = "home_end";
      outgoing = { kind: "progress", milestone: "collapse_ascended" };
    } else if (requestedPhase === "home_end") {
      state.stage = "home_end";
      const last = rows().length - 1;
      dispatchKey(rows()[1], "End");
      await waitFor(() => document.activeElement === rows()[last], "End to reach the last row");
      checkTabStopInvariant(last);
      dispatchKey(rows()[last], "Home");
      await waitFor(() => document.activeElement === rows()[0], "Home to reach the first row");
      checkTabStopInvariant(0);
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
      checkTabStopInvariant(3);
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
        checkTabStopInvariant(2);
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
    } else if (
      requestedPhase === "search_result_opens" &&
      // TEMP task 016 probe: skip contract (a) so contract (b) runs before the fix.
      (outgoing = advance("search_result_editor_focused", "new_file_focuses"))
    ) {
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
        if (state.errorToastSeen) throw new Error("an error toast appeared");
        state.milestones.push("tree_enter_refocused_after_new_file");
        outgoing = finish(true);
      } else {
        outgoing = { kind: "pending" };
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
