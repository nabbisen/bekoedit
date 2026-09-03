// Proves the pin-registry protocol stays one canonical shape across both
// drivers, even though it is duplicated as text (RFC-044 slice-1 §3/§9):
// the handoff calls the JS-side pin registry protocol shared, but driver.js
// itself is frozen for this slice ("driver.js... unchanged", §4/§7/§9), so
// there is no file it is safe to factor a shared fragment out of. This test
// is the substitute for a single copy: extract the pin-registry-relevant
// block from each driver, normalise the one expected difference (each
// driver's own phase name list), and assert the rest is identical. A future
// edit to either copy that silently diverges from the other fails this
// test, naming the mismatch.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const driverJs = readFileSync(
  new URL("../../src/webview_smoke/driver.js", import.meta.url),
  "utf8",
);
const shellBehaviourJs = readFileSync(
  new URL("../../src/webview_smoke/shell_behaviour_driver.js", import.meta.url),
  "utf8",
);

/** Extracts the substring between two anchors (both required to exist,
 * exactly once, in `source`) and normalises the one expected difference:
 * each driver's own phase-name array literal used in
 * `.includes(request.releasePhase)`/`.includes(requestedPhase)` checks. */
function extractPinRegistryProtocol(source, label) {
  const startAnchor = "let pinRegistry = window[pinKey];";
  const endAnchor = 'throw new Error("unexpected prior evaluator pin");';
  const start = source.indexOf(startAnchor);
  const end = source.indexOf(endAnchor);
  assert.ok(start !== -1, `${label}: start anchor not found`);
  assert.ok(end !== -1, `${label}: end anchor not found`);
  const block = source.slice(start, end + endAnchor.length);
  return block
    .replaceAll(/!?\["[a-z_]+"(?:, ?"[a-z_]+")*\]\.includes\(/g, "PHASE_LIST.includes(")
    .replaceAll(/!?phases\.includes\(/g, "PHASE_LIST.includes(");
}

/** Extracts the footer -- report/acknowledgement exchange, pin creation,
 * and the typed completion return. No per-driver differences expected
 * here at all. */
function extractFooter(source, label) {
  const startAnchor = "const report = {";
  const anchorCount = source.split(startAnchor).length - 1;
  assert.equal(anchorCount, 1, `${label}: expected exactly one report object literal`);
  return source.slice(source.indexOf(startAnchor));
}

test("the pin-registry setup/release block is the same protocol in both drivers", () => {
  const fromDriver = extractPinRegistryProtocol(driverJs, "driver.js");
  const fromShellBehaviour = extractPinRegistryProtocol(shellBehaviourJs, "shell_behaviour_driver.js");
  assert.equal(
    fromShellBehaviour,
    fromDriver,
    "shell_behaviour_driver.js's pin-registry block has drifted from driver.js's",
  );
});

test("the report/acknowledgement/pin-set footer is the same protocol in both drivers", () => {
  const fromDriver = extractFooter(driverJs, "driver.js");
  const fromShellBehaviour = extractFooter(shellBehaviourJs, "shell_behaviour_driver.js");
  assert.equal(
    fromShellBehaviour,
    fromDriver,
    "shell_behaviour_driver.js's footer has drifted from driver.js's",
  );
});

test("both drivers validate the request with the same shape, modulo their own phase list", () => {
  const requestCheck = (source) => {
    const start = source.indexOf("if (\n    request?.protocolVersion");
    const end = source.indexOf('throw new Error("invalid phase request");');
    assert.ok(start !== -1 && end !== -1);
    return source
      .slice(start, end + 'throw new Error("invalid phase request");'.length)
      .replaceAll(/!?\["[a-z_]+"(?:, ?"[a-z_]+")*\]\.includes\(/g, "PHASE_LIST.includes(")
      .replaceAll(/!?phases\.includes\(/g, "PHASE_LIST.includes(");
  };
  assert.equal(requestCheck(shellBehaviourJs), requestCheck(driverJs));
});
