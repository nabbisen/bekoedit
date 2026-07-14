export const BRIDGE_SCHEMA_VERSION = 2;

const sameIdentity = (left, right) => Boolean(left && right)
  && left.instanceId === right.instanceId
  && left.editorId === right.editorId
  && left.documentId === right.documentId
  && left.epoch === right.epoch;

const event = (type, fields = {}) => ({
  type,
  protocolVersion: BRIDGE_SCHEMA_VERSION,
  ...fields,
});

export function createLifecycleAdapter(deps) {
  let view = null;
  let identity = null;
  let revision = 0;
  let sequence = 0;
  let composing = false;
  let holdOperationId = null;
  let latestSnapshotOperationId = 0;
  let lastResume = null;
  const consumedTakeovers = new Set();

  const emit = (payload) => deps.emit(payload) !== false;
  const emitFailure = (request, reason) => {
    const common = {
      operationId: request.operationId,
      ...(request.identity ? { identity: request.identity } : {}),
    };
    switch (request.type) {
      case "probeBundle":
        return emit(event("bundleFailed", { operationId: request.operationId, reason }));
      case "installRelay":
        return emit(event("relayFailed", { ...common, reason }));
      case "initEditor":
        return emit(event("initFailed", { ...common, reason }));
      case "requestSnapshot":
        return emit(event("snapshotBlocked", { ...common, reason }));
      case "resumeEditing":
        return emit(event("resumeFailed", {
          ...common,
          snapshotOperationId: request.snapshotOperationId,
          reason,
        }));
      case "applyDocument":
        return emit(event("applyDocumentFailed", {
          operationId: request.operationId,
          identity: request.oldIdentity,
          reason,
        }));
      case "destroyEditor":
        return emit(event("destroyFailed", { ...common, reason }));
      default:
        return false;
    }
  };

  const requireVersion = (request) => {
    if (request.protocolVersion === BRIDGE_SCHEMA_VERSION) return true;
    emitFailure(request, "unsupportedVersion");
    return false;
  };

  const requireCurrent = (request) => {
    if (view && sameIdentity(identity, request.identity)) return true;
    emitFailure(request, view ? "identityMismatch" : "editorUnavailable");
    return false;
  };

  const destroyCurrent = () => {
    deps.cancelPendingChange();
    if (view) deps.destroyView(view);
    view = null;
    compositionReset();
  };

  const compositionReset = () => {
    composing = false;
    holdOperationId = null;
    latestSnapshotOperationId = 0;
    lastResume = null;
  };

  const validTakeover = (permit, replacement) => Boolean(
    permit
    && identity
    && permit.retiredInstanceId === identity.instanceId
    && permit.replacementInstanceId === replacement.instanceId
    && !consumedTakeovers.has(permit.nonce)
  );

  const initEditor = (request) => {
    if (view && sameIdentity(identity, request.identity)) {
      return emit(event("editorReady", {
        operationId: request.operationId,
        identity,
        revision,
        reused: true,
      }));
    }
    if (view && identity.instanceId === request.identity.instanceId) {
      return emitFailure(request, "identityMismatch");
    }
    if (view) {
      if (!validTakeover(request.takeover, request.identity)) {
        return emitFailure(request, "instanceAlreadyActive");
      }
      consumedTakeovers.add(request.takeover.nonce);
      destroyCurrent();
    }
    const parent = deps.getContainer(request.containerId);
    if (!parent) return emitFailure(request, "missingContainer");
    identity = { ...request.identity };
    revision = request.revision;
    sequence = 0;
    compositionReset();
    try {
      view = deps.createView(parent, request.text, () => holdOperationId !== null);
    } catch (_error) {
      view = null;
      identity = null;
      return emitFailure(request, "bridgeError");
    }
    return emit(event("editorReady", {
      operationId: request.operationId,
      identity,
      revision,
      reused: false,
    }));
  };

  const requestSnapshot = (request) => {
    if (!requireCurrent(request)) return false;
    if (composing) return emitFailure(request, "compositionActive");
    if (request.operationId < latestSnapshotOperationId) {
      return emitFailure(request, "identityMismatch");
    }
    deps.cancelPendingChange();
    latestSnapshotOperationId = request.operationId;
    holdOperationId = request.operationId;
    sequence += 1;
    return emit(event("snapshot", {
      operationId: request.operationId,
      identity,
      seq: sequence,
      text: deps.getText(view),
      composing: false,
    }));
  };

  const resumeEditing = (request) => {
    if (lastResume && lastResume.operationId === request.operationId) {
      return emit(lastResume.result);
    }
    if (!requireCurrent(request)) return false;
    if (holdOperationId !== null && holdOperationId !== request.snapshotOperationId) {
      return emitFailure(request, "identityMismatch");
    }
    if (holdOperationId === null
        && request.snapshotOperationId < latestSnapshotOperationId) {
      return emitFailure(request, "identityMismatch");
    }
    const wasHeld = holdOperationId === request.snapshotOperationId;
    holdOperationId = null;
    revision = request.revision;
    const result = event("editingResumed", {
      operationId: request.operationId,
      identity,
      snapshotOperationId: request.snapshotOperationId,
      revision,
      wasHeld,
    });
    lastResume = { operationId: request.operationId, result };
    return emit(result);
  };

  const applyDocument = (request) => {
    const matched = { ...request, identity: request.oldIdentity };
    if (!requireCurrent(matched)) return false;
    if (composing) return emitFailure(request, "compositionActive");
    deps.cancelPendingChange();
    try {
      deps.replaceDocument(view, request.text);
    } catch (_error) {
      return emitFailure(request, "bridgeError");
    }
    holdOperationId = null;
    sequence = 0;
    revision = request.revision;
    identity = { ...identity, epoch: request.newEpoch };
    return emit(event("documentApplied", {
      operationId: request.operationId,
      identity,
      revision,
    }));
  };

  const destroyEditor = (request) => {
    if (!view) {
      return emit(event("destroyed", {
        operationId: request.operationId,
        identity: request.identity,
      }));
    }
    if (!requireCurrent(request)) return false;
    const destroyed = identity;
    destroyCurrent();
    identity = null;
    return emit(event("destroyed", {
      operationId: request.operationId,
      identity: destroyed,
    }));
  };

  const dispatch = (request) => {
    if (!request || !requireVersion(request)) return false;
    switch (request.type) {
      case "probeBundle":
        return emit(event("bundleReady", { operationId: request.operationId }));
      case "installRelay":
        return emit(event("relayReady", {
          operationId: request.operationId,
          identity: request.identity,
        }));
      case "initEditor": return initEditor(request);
      case "requestSnapshot": return requestSnapshot(request);
      case "resumeEditing": return resumeEditing(request);
      case "applyDocument": return applyDocument(request);
      case "destroyEditor": return destroyEditor(request);
      default: return false;
    }
  };

  return {
    dispatch,
    publishChange(text) {
      if (!view || composing || holdOperationId !== null) return false;
      sequence += 1;
      return emit(event("change", {
        identity,
        seq: sequence,
        text,
        composing: false,
      }));
    },
    compositionStarted() {
      composing = true;
      deps.cancelPendingChange();
    },
    compositionEnded() { composing = false; },
    focus() { if (view) deps.focusView(view); },
    isHeld: () => holdOperationId !== null,
    currentIdentity: () => identity,
  };
}
