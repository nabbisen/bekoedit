const contains = (root, node) => Boolean(root && node)
  && (root === node || root.contains?.(node));

const sameFingerprint = (left, right) => left === right;

const sameIdentity = (left, right) => Boolean(left && right)
  && left.instanceId === right.instanceId
  && left.editorId === right.editorId
  && left.documentId === right.documentId
  && left.epoch === right.epoch;

export function consumeFocusRequest(registry, request, currentIdentity, focus, trace) {
  if (!request || !sameIdentity(currentIdentity, request.identity)) {
    registry.cancelThrough(request?.token ?? 0);
    trace("source.focus.rejected.identity");
    return false;
  }
  if (!registry.consume(request.token, request.fingerprint)) {
    registry.cancelThrough(request.token);
    trace("source.focus.rejected.guard");
    return false;
  }
  focus();
  trace("source.focus.consumed");
  return true;
}

export function createFocusGuardRegistry(deps) {
  let highestArmToken = 0;
  let cancelledThrough = 0;
  let currentGuard = null;
  let highestFingerprint = null;

  const removeCurrent = () => {
    if (!currentGuard) return;
    for (const [type, listener] of currentGuard.listeners) {
      deps.removeListener(type, listener, true);
    }
    currentGuard = null;
  };

  const negative = (request, reason) => ({
    token: request.token,
    armed: false,
    reason,
  });

  const arm = (request) => {
    if (!Number.isSafeInteger(request?.token) || request.token <= 0) {
      return negative(request ?? { token: 0 }, "invalidRequest");
    }
    if (request.token <= cancelledThrough) {
      return negative(request, "cancelledOrStale");
    }
    if (request.token < highestArmToken) {
      return negative(request, "cancelledOrStale");
    }
    if (request.token === highestArmToken) {
      if (currentGuard
          && currentGuard.token === request.token
          && sameFingerprint(currentGuard.fingerprint, request.fingerprint)) {
        return { token: request.token, armed: true, reason: null };
      }
      return negative(request, "duplicateMismatchOrRejected");
    }

    highestArmToken = request.token;
    highestFingerprint = request.fingerprint;
    const origin = deps.resolveOrigin(request);
    if (!origin?.isConnected) return negative(request, "originUnavailable");

    const guard = {
      token: request.token,
      fingerprint: request.fingerprint,
      origin,
      removalPolicy: request.removalPolicy,
      diverted: false,
      listeners: [],
    };
    const divert = () => { guard.diverted = true; };
    const onPointerDown = (event) => {
      if (!contains(guard.origin, event.target)) divert();
    };
    const onKeyDown = (event) => {
      if (event.key === "Tab") divert();
    };
    const onFocusIn = (event) => {
      if (contains(guard.origin, event.target)) return;
      const removableBodyFallback = guard.removalPolicy === "launchMayBeRemoved"
        && !guard.origin.isConnected
        && event.target === deps.body();
      if (!removableBodyFallback) divert();
    };
    guard.listeners = [
      ["pointerdown", onPointerDown],
      ["keydown", onKeyDown],
      ["focusin", onFocusIn],
    ];

    // A newer valid request owns the single registry slot. Listener teardown
    // is centralized so replacement cannot leave callbacks attached.
    removeCurrent();
    currentGuard = guard;
    for (const [type, listener] of guard.listeners) {
      deps.addListener(type, listener, true);
    }
    return { token: request.token, armed: true, reason: null };
  };

  const cancelThrough = (token) => {
    if (!Number.isSafeInteger(token) || token <= 0) return false;
    cancelledThrough = Math.max(cancelledThrough, token);
    if (currentGuard && currentGuard.token <= token) removeCurrent();
    return true;
  };

  const consume = (token, fingerprint) => {
    if (!currentGuard || currentGuard.token !== token) return false;
    const guard = currentGuard;
    const active = deps.activeElement();
    const originActive = guard.origin.isConnected && contains(guard.origin, active);
    const removedBodyFallback = guard.removalPolicy === "launchMayBeRemoved"
      && !guard.origin.isConnected
      && active === deps.body();
    const permitted = !guard.diverted
      && sameFingerprint(guard.fingerprint, fingerprint)
      && (originActive || removedBodyFallback);
    removeCurrent();
    cancelledThrough = Math.max(cancelledThrough, token);
    return permitted;
  };

  return {
    arm,
    cancelThrough,
    consume,
    inspect: () => ({
      highestArmToken,
      highestFingerprint,
      cancelledThrough,
      currentToken: currentGuard?.token ?? null,
    }),
  };
}

function isVisible(element) {
  if (!element?.isConnected) return false;
  const rects = element.getClientRects?.();
  return !rects || rects.length > 0;
}

export function createBrowserFocusGuardRegistry(document) {
  const resolveOrigin = (request) => {
    const active = document.activeElement;
    if (request.invocation === "pointer") {
      // WebKitGTK does not consistently move document.activeElement to a
      // button on pointer click. The Rust click handler already supplies the
      // control's unique launch ID, so resolve that exact visible control
      // without depending on browser focus policy.
      const launches = [...document.querySelectorAll("[data-source-focus-launch]")]
        .filter((element) => element.dataset.sourceFocusLaunch === request.launchId)
        .filter(isVisible);
      return launches.length === 1 ? launches[0] : null;
    }
    const selector = `[data-source-focus-launch-region="${request.currentMode}"]`;
    if (active && active !== document.body) {
      const region = active.closest?.(selector);
      return region && contains(region, active) ? region : null;
    }
    if (request.currentMode !== "preview") return null;
    const regions = [...document.querySelectorAll(selector)].filter(isVisible);
    return regions.length === 1 ? regions[0] : null;
  };
  return createFocusGuardRegistry({
    resolveOrigin,
    activeElement: () => document.activeElement,
    body: () => document.body,
    addListener: (...args) => document.addEventListener(...args),
    removeListener: (...args) => document.removeEventListener(...args),
  });
}
