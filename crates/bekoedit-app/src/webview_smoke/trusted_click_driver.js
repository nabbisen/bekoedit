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
  const phases = ["proof_of_trust", "tree_row_focus", "backlink_focus", "mode_tab_focus"];
  const request = await dioxus.recv();
  const requestedPhase = request?.phase;
  const exchangeId = request?.exchangeId;

  // Task 023 review (root-cause-fixed) §4.3 / task 025 §2.3: a rejection
  // here used to throw before any dioxus.send() at all, so Rust's own
  // eval.recv() waited out the shared transport's full 5 s cap in silence.
  // Every pre-try check now sends a diagnostic terminal report first, so a
  // rejection here is fast and named instead of a 5-second silence.
  const failEarly = (message) => {
    dioxus.send({
      protocolVersion,
      exchangeId: exchangeId ?? 0,
      phase: requestedPhase ?? "unknown",
      releasedExchangeId: null,
      releasedPhase: null,
      kind: "terminal",
      result: {
        ok: false,
        stage: "invalid_request",
        marker,
        milestones: [],
        errorToastSeen: false,
        error: message,
      },
    });
    throw new Error(message);
  };

  if (
    request?.protocolVersion !== protocolVersion ||
    !Number.isSafeInteger(exchangeId) ||
    exchangeId <= 0 ||
    !phases.includes(requestedPhase)
  ) {
    failEarly(
      `invalid phase request: protocolVersion=${JSON.stringify(request?.protocolVersion)} ` +
        `exchangeId=${JSON.stringify(exchangeId)} phase=${JSON.stringify(requestedPhase)}`,
    );
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
    failEarly("incompatible trusted-click evaluator pin registry");
  }

  const hasReleaseId = request.releaseExchangeId !== null;
  const hasReleasePhase = request.releasePhase !== null;
  if (hasReleaseId !== hasReleasePhase) {
    failEarly("incomplete prior evaluator pin release");
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
      failEarly("prior evaluator pin did not match release request");
    }
    releasedExchangeId = request.releaseExchangeId;
    releasedPhase = request.releasePhase;
    pinRegistry.current = null;
  } else if (pinRegistry.current !== null) {
    failEarly("unexpected prior evaluator pin");
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

  // Task 029: what the page looks like, in one line, so a timeout can say
  // which of "the click never landed", "no document opened", "the editor never
  // became ready", "focus was refused" or "focus was lost" it was. Read only
  // by `observe` and `timeoutMessage`; it changes nothing on the page.
  const describeElement = (element) => {
    if (!element) return "none";
    const tag = String(element.tagName ?? "element").toLowerCase();
    const id = element.id ? `#${element.id}` : "";
    const classes = element.className
      ? `.${String(element.className).trim().split(/\s+/).join(".")}`
      : "";
    return `${tag}${id}${classes}`;
  };
  const describeState = () => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    const active = document.activeElement ?? null;
    const fileName = document.querySelector(".file-name")?.textContent?.trim() ?? null;
    const rows = [...(document.querySelectorAll?.(".tree-row.tree-file") ?? [])]
      .map((row) => `${row.textContent?.trim()}(selected=${row.getAttribute?.("aria-selected")})`)
      .join(",");
    const modeTabs = [...(document.querySelectorAll?.('[role="tab"][aria-selected="true"]') ?? [])]
      .map((tab) => tab.getAttribute?.("data-source-focus-launch"))
      .join(",");
    const status = host?.querySelector?.(".source-editor-status")?.textContent?.trim() ?? null;
    return [
      `document.hasFocus()=${document.hasFocus?.()}`,
      `activeElement=${describeElement(active)} inEditorHost=${Boolean(active && host?.contains?.(active))}`,
      `openFile=${fileName} treeRows=[${rows}] selectedModeTab=${modeTabs || "none"}`,
      `editor: view=${Boolean(view)} connected=${view?.dom?.isConnected} hasFocus=${view?.hasFocus} ` +
        `host=${Boolean(host)} statusMarker=${status}`,
    ].join("; ");
  };
  // Records the state at the first poll after a phase's click, and the first
  // time it differs, so a timeout can say where the wait started and where it
  // ended. Keyed by phase, so each phase starts its own record.
  const observe = () => {
    const now = describeState();
    const watch = state.watch;
    if (watch?.phase !== requestedPhase) {
      state.watch = { phase: requestedPhase, firstAt: performance.now(), first: now, changedAt: null, changed: null };
    } else if (watch.changedAt === null && now !== watch.first) {
      watch.changedAt = performance.now();
      watch.changed = now;
    }
  };
  const timeoutMessage = (stage) => {
    const watch = state.watch;
    const changed =
      watch?.changedAt == null
        ? "no change in the whole wait"
        : `first change ${Math.round(watch.changedAt - watch.firstAt)} ms after the first poll, to: ${watch.changed}`;
    return (
      `timed out at ${stage}. At the first poll after the click: ${watch?.first ?? "not recorded"}. ` +
      `At the timeout: ${describeState()}. ${changed}`
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
    observe();

    if (requestedPhase === "proof_of_trust") {
      // The Rust side has already sent a real XTEST click at
      // #app-menu-trigger's screen position before this exchange was
      // requested. A synthetic click would not do this: browsers withhold
      // the default focus action from an untrusted, script-dispatched
      // click event, so document.activeElement would not move.
      if (timedOut()) throw new Error(timeoutMessage("trusted_click_focused_default_target"));
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
      if (timedOut()) throw new Error(timeoutMessage("tree_row_trusted_click_focused_editor"));
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
      if (timedOut()) throw new Error(timeoutMessage("backlink_trusted_click_focused_editor"));
      if (!editorFocused("parent.md")) {
        outgoing = { kind: "pending" };
      } else {
        if (state.errorToastSeen) throw new Error("an error toast appeared");
        state.milestones.push("backlink_trusted_click_focused_editor");
        advance("mode_tab_focus", "mode_tab_trusted_click_focused_editor");
        outgoing = {
          kind: "progress",
          milestone: "backlink_trusted_click_focused_editor",
        };
      }
    } else if (requestedPhase === "mode_tab_focus") {
      // Rust has already switched parent.md to Form mode with a real
      // XTEST click, then sent a second real XTEST click at the Text
      // mode tab's rect. §C, the check this file exists for: the editor
      // takes focus. Terminal.
      if (timedOut()) throw new Error(timeoutMessage("mode_tab_trusted_click_focused_editor"));
      const active = document.querySelector(
        '[data-source-focus-launch="mode-text"].active[aria-selected="true"]',
      );
      if (!active || !editorFocused("parent.md")) {
        outgoing = { kind: "pending" };
      } else {
        if (state.errorToastSeen) throw new Error("an error toast appeared");
        state.milestones.push("mode_tab_trusted_click_focused_editor");
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
  // Task 025 review §3.1: a thrown message does not survive eval.join() on
  // WebKitGTK (a bare EvalError::Communication), but a returned value does,
  // so a failure after the acknowledgement is returned in the completion.
  const completionFailure = (acknowledgementProcessed, error) => ({
    protocolVersion,
    exchangeId,
    phase: requestedPhase,
    kind: report.kind,
    acknowledgementProcessed,
    evaluatorPinned: false,
    error,
  });
  const acknowledgement = await dioxus.recv();
  if (
    acknowledgement?.protocolVersion !== protocolVersion ||
    acknowledgement?.exchangeId !== exchangeId ||
    acknowledgement?.phase !== requestedPhase ||
    acknowledgement?.kind !== report.kind
  ) {
    return completionFailure(false, "invalid phase acknowledgement");
  }

  if (pinRegistry.current !== null) {
    return completionFailure(true, "trusted-click evaluator pin was already occupied");
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
