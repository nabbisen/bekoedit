const contains = (root, node) => Boolean(root && node)
  && (root === node || root.contains?.(node));

const sameFingerprint = (left, right) => left === right;

const sameIdentity = (left, right) => Boolean(left && right)
  && left.instanceId === right.instanceId
  && left.editorId === right.editorId
  && left.documentId === right.documentId
  && left.epoch === right.epoch;

export const FOCUS_GUARD_PROTOCOL_VERSION = 2;
const FOCUS_GUARD_REGISTRY_KEY = "__bkFocusGuards";

const unevaluatedDiagnostic = (reason, tokenRelation = "notEvaluated") => ({
  outcome: "rejected",
  reason,
  tokenRelation,
  diversion: "notEvaluated",
  fingerprintRelation: "notEvaluated",
  originConnection: "notEvaluated",
  activeElementRelation: "notEvaluated",
  removalPolicy: "notEvaluated",
  removedBodyFallback: "notEvaluated",
});

const emitTraceSafely = (trace, name, details) => {
  try {
    trace(name, details);
  } catch (_error) {
    // Diagnostic delivery is never allowed to change focus authority.
  }
};

const safeFocusToken = (token) => Number.isSafeInteger(token) && token > 0 ? token : null;

export function consumeFocusRequest(registry, request, currentIdentity, focus, trace) {
  if (!request) {
    registry.cancelThrough(0);
    emitTraceSafely(trace, "source.focus.rejected.identity", {
      focusToken: null,
      focusGuardDiagnostic: unevaluatedDiagnostic("requestMissing"),
    });
    return false;
  }
  if (!sameIdentity(currentIdentity, request.identity)) {
    registry.cancelThrough(request?.token ?? 0);
    emitTraceSafely(trace, "source.focus.rejected.identity", {
      focusToken: safeFocusToken(request.token),
      focusGuardDiagnostic: unevaluatedDiagnostic("identityMismatch"),
    });
    return false;
  }
  const consumed = registry.consumeDiagnostic(request.token, request.fingerprint);
  if (!consumed.accepted) {
    registry.cancelThrough(request.token);
    emitTraceSafely(trace, "source.focus.rejected.guard", {
      focusToken: safeFocusToken(request.token),
      focusGuardDiagnostic: consumed.diagnostic,
    });
    return false;
  }
  focus();
  emitTraceSafely(trace, "source.focus.consumed", {
    focusToken: safeFocusToken(request.token),
    focusGuardDiagnostic: consumed.diagnostic,
  });
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
      diversionReason: null,
      listeners: [],
    };
    const divert = (reason) => {
      if (guard.diversionReason === null) guard.diversionReason = reason;
    };
    const onPointerDown = (event) => {
      if (!contains(guard.origin, event.target)) divert("pointer");
    };
    const onKeyDown = (event) => {
      if (event.key === "Tab") divert("tab");
    };
    const onFocusIn = (event) => {
      if (contains(guard.origin, event.target)) return;
      const removableBodyFallback = guard.removalPolicy === "launchMayBeRemoved"
        && !guard.origin.isConnected
        && event.target === deps.body();
      if (!removableBodyFallback) divert("focusIn");
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

  const consumeDiagnostic = (token, fingerprint) => {
    if (!currentGuard) {
      return {
        accepted: false,
        diagnostic: unevaluatedDiagnostic("missingGuard", "noGuard"),
      };
    }
    if (currentGuard.token !== token) {
      return {
        accepted: false,
        diagnostic: unevaluatedDiagnostic("tokenMismatch", "mismatch"),
      };
    }
    const guard = currentGuard;
    const active = deps.activeElement();
    const originConnection = guard.origin.isConnected ? "connected" : "disconnected";
    const activeElementRelation = contains(guard.origin, active)
      ? "origin"
      : active === deps.body()
        ? "body"
        : active
          ? "other"
          : "none";
    const fingerprintRelation = sameFingerprint(guard.fingerprint, fingerprint)
      ? "equal"
      : "mismatch";
    const originActive = originConnection === "connected"
      && activeElementRelation === "origin";
    const removedBodyFallback = guard.removalPolicy === "launchMayBeRemoved"
      && originConnection === "disconnected"
      && activeElementRelation === "body";
    const removalPolicy = guard.removalPolicy === "launchMayBeRemoved"
      || guard.removalPolicy === "launchMustRemain"
      ? guard.removalPolicy
      : "notEvaluated";
    const permitted = guard.diversionReason === null
      && fingerprintRelation === "equal"
      && (originActive || removedBodyFallback);
    const reason = guard.diversionReason === "pointer"
      ? "divertedPointer"
      : guard.diversionReason === "tab"
        ? "divertedTab"
        : guard.diversionReason === "focusIn"
          ? "divertedFocusIn"
          : fingerprintRelation === "mismatch"
            ? "fingerprintMismatch"
            : !originActive && !removedBodyFallback
              ? "activeElementIneligible"
              : "accepted";
    const diagnostic = {
      outcome: permitted ? "accepted" : "rejected",
      reason,
      tokenRelation: "match",
      diversion: guard.diversionReason ?? "none",
      fingerprintRelation,
      originConnection,
      activeElementRelation,
      removalPolicy,
      removedBodyFallback: removedBodyFallback ? "eligible" : "ineligible",
    };
    removeCurrent();
    cancelledThrough = Math.max(cancelledThrough, token);
    return { accepted: permitted, diagnostic };
  };
  const consume = (token, fingerprint) => consumeDiagnostic(token, fingerprint).accepted;

  return {
    arm,
    cancelThrough,
    consume,
    consumeDiagnostic,
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

function compatibleBrowserRegistry(candidate) {
  return candidate?.protocolVersion === FOCUS_GUARD_PROTOCOL_VERSION
    && typeof candidate.arm === "function"
    && typeof candidate.cancelThrough === "function"
    && typeof candidate.consume === "function"
    && typeof candidate.consumeDiagnostic === "function"
    && typeof candidate.inspect === "function";
}

export function installBrowserFocusGuardRegistry(globalObject, document) {
  const existing = globalObject[FOCUS_GUARD_REGISTRY_KEY];
  if (existing !== undefined) {
    return compatibleBrowserRegistry(existing) ? existing : null;
  }

  const registry = createBrowserFocusGuardRegistry(document);
  const singleton = Object.freeze({
    protocolVersion: FOCUS_GUARD_PROTOCOL_VERSION,
    arm: registry.arm,
    cancelThrough: registry.cancelThrough,
    consume: registry.consume,
    consumeDiagnostic: registry.consumeDiagnostic,
    inspect: registry.inspect,
  });
  Object.defineProperty(globalObject, FOCUS_GUARD_REGISTRY_KEY, {
    value: singleton,
    configurable: false,
    enumerable: false,
    writable: false,
  });
  return singleton;
}
