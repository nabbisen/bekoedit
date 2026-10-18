return (async () => {
  const marker = "RFC041_WEBVIEW_SMOKE_MARKER";
  const stateKey = "__bkWebViewSmokeState";
  const pinKey = "__bkWebViewSmokeEvalPin";
  const protocolVersion = 2;
  const pinProtocolVersion = 1;
  const phases = ["launch", "editor", "preview"];
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
    failEarly("incompatible smoke evaluator pin registry");
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
  const click = (element) =>
    element.dispatchEvent(
      new MouseEvent("click", {
        view: window,
        bubbles: true,
        cancelable: true,
        button: 0,
      }),
    );

  const createState = () => {
    const state = {
      protocolVersion: 1,
      phase: "launch",
      stage: "observer_install",
      deadline: null,
      milestones: ["observer_installed"],
      errorToastSeen: Boolean(document.querySelector(".toast-error")),
      // Task 061: when the New click landed, and when the editor view was
      // first seen, on `performance.now()`'s clock. Read only by the editor
      // phase's timeout diagnosis.
      clickedAt: null,
      viewSeenAt: null,
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
  const finish = (ok, error = null, diagnosis = null) => {
    const result = {
      ok,
      stage: state.stage,
      marker,
      milestones: [...state.milestones],
      errorToastSeen: state.errorToastSeen,
      error,
      // Task 061: present only on a failure that can say why.
      ...(diagnosis ? { diagnosis } : {}),
    };
    state.observer.disconnect();
    delete window[stateKey];
    return { kind: "terminal", result };
  };
  const timedOut = () => state.deadline !== null && performance.now() >= state.deadline;

  // Task 061: what the page looked like when the editor phase ran out of
  // time. The wait is for five conditions at once, and "timed out" alone
  // cannot say which one was missing. Everything here only reads the page,
  // and none of it reads text: tags, ids, classes, fixed `data-source-focus-*`
  // labels and booleans.
  const editorConditions = () => {
    const view = window.__bk?._view;
    const host = document.querySelector('[data-source-focus-launch-region="text"]');
    // `null` is "cannot be told": a condition about the view, or about the
    // host's contents, when there is no view or no host to ask.
    return {
      view,
      host,
      viewExists: Boolean(view),
      viewConnected: view ? Boolean(view.dom?.isConnected) : null,
      viewHasFocus: view ? Boolean(view.hasFocus) : null,
      hostExists: Boolean(host),
      hostHasNoStatusMarker: host ? !host.querySelector?.(".source-editor-status") : null,
    };
  };
  const classNames = (element) =>
    typeof element.className === "string" && element.className.trim() !== ""
      ? `.${element.className.trim().split(/\s+/).join(".")}`
      : "";
  const describeActiveElement = () => {
    const active = document.activeElement ?? null;
    if (!active) return "none";
    const id = active.id ? `#${active.id}` : "";
    const focusLabels = (active.getAttributeNames?.() ?? [])
      .filter((name) => name.startsWith("data-source-focus-"))
      .map((name) => `[${name}=${active.getAttribute(name)}]`)
      .join("");
    return `${String(active.tagName ?? "element").toLowerCase()}${id}${focusLabels}${classNames(active)}`;
  };
  const editorDiagnosis = (conditions) => {
    const now = performance.now();
    const after = (at) => `${Math.round(at - state.clickedAt)} ms`;
    const marker = conditions.host?.querySelector?.(".source-editor-status") ?? null;
    const shown = (value) => (value === null ? "n/a" : value);
    return [
      `editor conditions: window.__bk._view=${conditions.viewExists} ` +
        `view.dom.isConnected=${shown(conditions.viewConnected)} view.hasFocus=${shown(conditions.viewHasFocus)} ` +
        `textHost=${conditions.hostExists} hostHasNoStatusMarker=${shown(conditions.hostHasNoStatusMarker)}`,
      `document.hasFocus()=${document.hasFocus?.()}; activeElement=${describeActiveElement()}`,
      `statusMarker=${marker ? classNames(marker) || "present" : "none"}`,
      `timing: ${after(now)} after new_clicked (deadline ${after(state.deadline)}); ` +
        `editor view first seen ${state.viewSeenAt === null ? "never" : `${after(state.viewSeenAt)} after new_clicked`}`,
    ];
  };

  let outgoing;
  try {
    if (state.protocolVersion !== 1 || requestedPhase !== state.phase) {
      throw new Error(
        `phase mismatch: requested ${requestedPhase}, current ${state.phase}`,
      );
    }

    if (requestedPhase === "launch") {
      state.stage = "start_visible";
      const newButton = document.querySelector(
        '[data-source-focus-launch="start-new"]',
      );
      if (!newButton) {
        outgoing = { kind: "pending" };
      } else {
        state.milestones.push("start_visible");
        if (!click(newButton)) throw new Error("New click was not accepted");
        state.milestones.push("new_clicked");
        state.deadline = performance.now() + 15000;
        state.clickedAt = state.deadline - 15000;
        state.phase = "editor";
        outgoing = {
          kind: "progress",
          milestone: "new_clicked",
        };
      }
    } else if (requestedPhase === "editor") {
      state.stage = "editor_ready_focused";
      const conditions = editorConditions();
      if (conditions.viewExists && state.viewSeenAt === null) {
        state.viewSeenAt = performance.now();
      }
      if (timedOut()) {
        const timeout = new Error("timed out at editor_ready_focused");
        timeout.diagnosis = editorDiagnosis(conditions);
        throw timeout;
      }
      const view = window.__bk?._view;
      const host = document.querySelector(
        '[data-source-focus-launch-region="text"]',
      );
      const ready = view &&
        view.dom?.isConnected &&
        view.hasFocus &&
        host &&
        !host.querySelector(".source-editor-status");
      if (!ready) {
        outgoing = { kind: "pending" };
      } else {
        state.milestones.push("editor_ready_focused");
        state.stage = "edit_dispatched";
        view.dispatch({
          changes: { from: view.state.doc.length, insert: marker },
        });
        state.milestones.push("edit_dispatched");
        const previewButton = document.querySelector(
          '[data-source-focus-launch="mode-preview"]',
        );
        if (!previewButton) throw new Error("Preview control is missing");
        if (!click(previewButton)) throw new Error("Preview click was not accepted");
        state.milestones.push("preview_clicked");
        state.phase = "preview";
        outgoing = {
          kind: "progress",
          milestone: "preview_clicked",
        };
      }
    } else if (requestedPhase === "preview") {
      state.stage = "preview_verified";
      if (timedOut()) throw new Error("timed out at preview_verified");
      const article = document.querySelector("article.preview");
      const active = document.querySelector(
        '[data-source-focus-launch="mode-preview"].active[aria-selected="true"]',
      );
      if (!article?.textContent?.includes(marker) || !active) {
        outgoing = { kind: "pending" };
      } else {
        if (state.errorToastSeen) throw new Error("an error toast appeared");
        state.milestones.push("preview_verified");
        outgoing = finish(true);
      }
    } else {
      throw new Error(`unknown phase: ${requestedPhase}`);
    }
  } catch (error) {
    outgoing = finish(false, String(error), error?.diagnosis ?? null);
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
    return completionFailure(true, "smoke evaluator pin was already occupied");
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
