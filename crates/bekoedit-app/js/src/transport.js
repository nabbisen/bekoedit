export function relayForGeneration(root, relayName, generation) {
  const relay = root[relayName];
  return typeof relay === "function" && relay.__bkGeneration === generation
    ? relay
    : null;
}

export function dispatchForRelayGeneration(
  root,
  relayName,
  generation,
  dispatch,
  request,
) {
  if (!relayForGeneration(root, relayName, generation)) return false;
  dispatch(request);
  return true;
}
