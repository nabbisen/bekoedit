return (async () => {
  // Task 023: this driver never calls .click() or dispatches a synthetic
  // MouseEvent, anywhere, for any purpose -- setup included. Every click in
  // this run, without exception, is a real XTEST click the Rust side sends
  // through xdotool before requesting the phase that observes its outcome.
  // This file only watches for what a click already did.
  const marker = "TASK023_TRUSTED_CLICK_MARKER";
  const stateKey = "__bkTrustedClickState";
  const pinKey = "__bkTrustedClickEvalPin";
  const protocolVersion = 2;
  const pinProtocolVersion = 1;
  const phases = ["proof_of_trust", "tree_row_focus", "backlink_focus", "no_op_terminal"];
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
    throw new Error("incompatible trusted-click evaluator pin registry");
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

  const containsErrorToast = (node) =>
    node?.nodeType === Node.ELEMENT_NODE &&
    (node.matches?.(".toast-error") || node.querySelector?.(".toast-error"));

  const editorFocused = (expectedFileName) => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    const fileName = document.querySelector(".file-name")?.textContent?.trim() ?? null;
    return Boolean(
      view &&
        view.dom?.isConnected &&
        view.hasFocus &&
        host &&
        !host.querySelector(".source-editor-status") &&
        (expectedFileName === null || fileName === expectedFileName),
    );
  };

  const createState = () => {
    const state = {
      protocolVersion: 1,
      phase: "proof_of_trust",
      stage: "trusted_click_focused_default_target",
      deadline: performance.now() + 10000,
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
  const timedOut = () => performance.now() >= state.deadline;
  const advance = (nextPhase, nextStage) => {
    state.phase = nextPhase;
    state.stage = nextStage;
    state.deadline = performance.now() + 10000;
  };

  let outgoing;
  try {
    if (state.protocolVersion !== 1 || requestedPhase !== state.phase) {
      throw new Error(
        `phase mismatch: requested ${requestedPhase}, current ${state.phase}`,
      );
    }

    if (requestedPhase === "proof_of_trust") {
      // The Rust side has already sent a real XTEST click at
      // #app-menu-trigger's screen position before this exchange was
      // requested. A synthetic click would not do this: browsers withhold
      // the default focus action from an untrusted, script-dispatched
      // click event, so document.activeElement would not move.
      if (timedOut()) throw new Error("timed out at trusted_click_focused_default_target");
      const trigger = document.getElementById("app-menu-trigger");
      if (document.activeElement !== trigger) {
        outgoing = { kind: "pending" };
      } else {
        state.milestones.push("trusted_click_focused_default_target");
        advance("tree_row_focus", "tree_row_trusted_click_focused_editor");
        outgoing = {
          kind: "progress",
          milestone: "trusted_click_focused_default_target",
        };
      }
    } else if (requestedPhase === "tree_row_focus") {
      // Rust has already sent a real XTEST click at child.md's tree-row
      // rect. §B item 1: the editor takes focus.
      if (timedOut()) throw new Error("timed out at tree_row_trusted_click_focused_editor");
      if (!editorFocused("child.md")) {
        outgoing = { kind: "pending" };
      } else {
        state.milestones.push("tree_row_trusted_click_focused_editor");
        advance("backlink_focus", "backlink_trusted_click_focused_editor");
        outgoing = {
          kind: "progress",
          milestone: "tree_row_trusted_click_focused_editor",
        };
      }
    } else if (requestedPhase === "backlink_focus") {
      // Rust has already opened the backlinks panel and sent a real XTEST
      // click at the backlink button's rect. §B item 2: the editor takes
      // focus, on parent.md (the backlink's source document).
      if (timedOut()) throw new Error("timed out at backlink_trusted_click_focused_editor");
      if (!editorFocused("parent.md")) {
        outgoing = { kind: "pending" };
      } else {
        if (state.errorToastSeen) throw new Error("an error toast appeared");
        state.milestones.push("backlink_trusted_click_focused_editor");
        advance("no_op_terminal", "no_op_terminal_reported");
        outgoing = {
          kind: "progress",
          milestone: "backlink_trusted_click_focused_editor",
        };
      }
    } else if (requestedPhase === "no_op_terminal") {
      // Diagnostic (task 023's 2026-09-23 finding review §4.3): no click
      // of its own, nothing to wait for -- reports success on its very
      // first query. §C (Form mode, click the Text tab) is not part of
      // this run; see phase.rs's own doc comment.
      state.milestones.push("no_op_terminal_reported");
      outgoing = finish(true);
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
    throw new Error("trusted-click evaluator pin was already occupied");
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
