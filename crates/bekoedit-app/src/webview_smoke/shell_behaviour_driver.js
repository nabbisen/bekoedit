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
      enterDispatched: false,
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

  let outgoing;
  try {
    if (state.protocolVersion !== 1 || requestedPhase !== state.phase) {
      throw new Error(
        `phase mismatch: requested ${requestedPhase}, current ${state.phase}`,
      );
    }

    await waitFor(
      () => rows().length >= 4,
      "the workspace tree to render its rows",
    );

    if (requestedPhase === "down_up") {
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
      // Expanding an unscanned directory triggers a real, async filesystem
      // scan through the tree's own coroutine (on_toggled -> ScanRequest ->
      // background thread -> on_loaded merge) -- not a synchronous state
      // flip, so this phase is multi-call and pollable (like enter_opens),
      // not a single blocking wait: a document::eval call has its own
      // outer deadline (the Rust-side evaluator timeout) shorter than the
      // scan can be relied on to finish within.
      state.stage = "expand_enter";
      if (timedOut()) {
        throw new Error(
          `timed out at expand_enter: row count is ${rows().length} (expected ${
            (state.expandBeforeCount ?? "?") + 1
          }), row 0 aria-expanded=${rows()[0]?.getAttribute("aria-expanded")}, ` +
            `activeElement is row ${rows().findIndex((row) => row === document.activeElement)}`,
        );
      }
      if (!state.expandDispatched) {
        state.expandBeforeCount = rows().length;
        dispatchKey(rows()[0], "ArrowRight");
        state.expandDispatched = true;
        state.deadline = performance.now() + 10000;
        outgoing = { kind: "pending" };
      } else if (!state.expandConfirmed) {
        if (rows().length !== state.expandBeforeCount + 1) {
          outgoing = { kind: "pending" };
        } else {
          if (rows()[0].getAttribute("aria-expanded") !== "true") {
            throw new Error("expanded row did not report aria-expanded=true");
          }
          if (document.activeElement !== rows()[0]) {
            throw new Error("expanding must not move focus off the directory row");
          }
          checkTabStopInvariant(0);
          state.expandConfirmed = true;
          dispatchKey(rows()[0], "ArrowRight");
          outgoing = { kind: "pending" };
        }
      } else if (document.activeElement !== rows()[1]) {
        outgoing = { kind: "pending" };
      } else {
        checkTabStopInvariant(1);
        state.milestones.push("expand_entered");
        state.phase = "collapse_ascend";
        outgoing = { kind: "progress", milestone: "expand_entered" };
      }
    } else if (requestedPhase === "collapse_ascend") {
      state.stage = "collapse_ascend";
      dispatchKey(rows()[1], "ArrowLeft");
      await waitFor(
        () => document.activeElement === rows()[0],
        "ArrowLeft to ascend from the child row to its parent directory",
      );
      checkTabStopInvariant(0);
      const before = rows().length;
      dispatchKey(rows()[0], "ArrowLeft");
      await waitFor(
        () => rows().length === before - 1,
        "ArrowLeft to collapse the expanded directory (one fewer row)",
      );
      if (rows()[0].getAttribute("aria-expanded") !== "false") {
        throw new Error("collapsed row did not report aria-expanded=false");
      }
      if (document.activeElement !== rows()[0]) {
        throw new Error("collapsing must not move focus off the directory row");
      }
      checkTabStopInvariant(0);
      state.milestones.push("collapse_ascended");
      state.phase = "home_end";
      outgoing = { kind: "progress", milestone: "collapse_ascended" };
    } else if (requestedPhase === "home_end") {
      state.stage = "home_end";
      const last = rows().length - 1;
      dispatchKey(rows()[0], "End");
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
      await waitFor(
        () => document.activeElement === rows()[2],
        "ArrowDown to reach the non-openable row (it must not be skipped)",
      );
      checkTabStopInvariant(2);
      const target = rows()[2];
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
      state.stage = "enter_opens";
      if (timedOut()) throw new Error("timed out at enter_opens");
      if (!state.enterDispatched) {
        dispatchKey(rows()[2], "ArrowUp");
        await waitFor(
          () => document.activeElement === rows()[1],
          "ArrowUp to return to the markdown row",
        );
        checkTabStopInvariant(1);
        dispatchKey(rows()[1], "Enter");
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
          if (state.errorToastSeen) throw new Error("an error toast appeared");
          state.milestones.push("enter_opened_editor_focused");
          outgoing = finish(true);
        }
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
