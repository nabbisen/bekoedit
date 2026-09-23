// Proves the pin-registry protocol stays one canonical shape across all
// three drivers, even though it is duplicated as text (RFC-044 slice-1 §3/§9,
// task 025 §2.5): the handoff calls the JS-side pin registry protocol
// shared, but driver.js itself is frozen for RFC-044 slice-1 ("driver.js...
// unchanged", §4/§7/§9), so there is no file it is safe to factor a shared
// fragment out of. This test is the substitute for a single copy: extract
// the pin-registry-relevant block from each driver, normalise the expected
// differences (each driver's own phase name list, and trusted_click's own
// evaluator label -- task 025 §2.5 found this one and it is reported, not
// silently generalised away, below), and assert the rest is identical. A
// future edit to any copy that silently diverges from the others fails this
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
const trustedClickJs = readFileSync(
  new URL("../../src/webview_smoke/trusted_click_driver.js", import.meta.url),
  "utf8",
);

const DRIVERS = [
  ["driver.js", driverJs],
  ["shell_behaviour_driver.js", shellBehaviourJs],
  ["trusted_click_driver.js", trustedClickJs],
];

/** Asserts `extract(source, label)` returns the same text for every driver,
 * comparing each to the first (driver.js) rather than the O(n^2) full
 * cross-product -- equal-to-the-same-thing is equal to each other. */
function assertSharedAcrossAllDrivers(extract, description) {
  const [[firstLabel, firstSource], ...rest] = DRIVERS;
  const reference = extract(firstSource, firstLabel);
  for (const [label, source] of rest) {
    assert.equal(
      extract(source, label),
      reference,
      `${label}'s ${description} has drifted from ${firstLabel}'s`,
    );
  }
}

/** Extracts the substring between two anchors (both required to exist,
 * exactly once, in `source`) and normalises the one expected difference:
 * each driver's own phase-name array literal used in
 * `.includes(request.releasePhase)`/`.includes(requestedPhase)` checks.
 * Task 025 §2.5 found one real difference beyond the phase list:
 * trusted_click_driver.js names its own evaluator ("incompatible
 * trusted-click evaluator pin registry") where the other two say "smoke" --
 * a deliberate per-run label (reported to the reviewer, not a bug),
 * normalised via `normalizeEvaluatorLabel` the same way the phase list is. */
function normalizeEvaluatorLabel(text) {
  return text.replaceAll(/(?:smoke|trusted-click) evaluator pin/g, "EVALUATOR_LABEL evaluator pin");
}

function extractPinRegistryProtocol(source, label) {
  const startAnchor = "let pinRegistry = window[pinKey];";
  const endAnchor = 'failEarly("unexpected prior evaluator pin");';
  const start = source.indexOf(startAnchor);
  const end = source.indexOf(endAnchor);
  assert.ok(start !== -1, `${label}: start anchor not found`);
  assert.ok(end !== -1, `${label}: end anchor not found`);
  const block = source.slice(start, end + endAnchor.length);
  return normalizeEvaluatorLabel(block.replaceAll(/!?phases\.includes\(/g, "PHASE_LIST.includes("));
}

/** Extracts the footer -- report/acknowledgement exchange, pin creation,
 * and the typed completion return. Same one real difference as the
 * pin-registry block: trusted_click_driver.js's own evaluator label in
 * "trusted-click evaluator pin was already occupied", normalised the same
 * way. No other per-driver difference is expected in this block. */
function extractFooter(source, label) {
  const startAnchor = "const report = {";
  const anchorCount = source.split(startAnchor).length - 1;
  assert.equal(anchorCount, 1, `${label}: expected exactly one report object literal`);
  return normalizeEvaluatorLabel(source.slice(source.indexOf(startAnchor)));
}

/** The declared value of `const <name> = ...;` at the top of a driver
 * (e.g. `const pinKey = "__bkWebViewSmokeEvalPin";` -> `"__bkWebViewSmokeEvalPin"`). */
function declaredConstant(source, name, label) {
  const match = source.match(new RegExp(`const ${name} = ([^;]+);`));
  assert.ok(match, `${label}: no \`const ${name} = ...;\` declaration found`);
  return match[1];
}

/** Task 025 §2.3: the failEarly helper itself, plus the request-shape check
 * that is every driver's first use of it -- one block, since the two
 * always change together. Normalises only the phase list. */
function extractFailEarlyAndRequestCheck(source, label) {
  const startAnchor = "// Task 023 review (root-cause-fixed) §4.3 / task 025 §2.3:";
  const endAnchor = "phase=${JSON.stringify(requestedPhase)}`,\n    );\n  }";
  const start = source.indexOf(startAnchor);
  const end = source.indexOf(endAnchor);
  assert.ok(start !== -1, `${label}: start anchor not found`);
  assert.ok(end !== -1, `${label}: end anchor not found`);
  const block = source.slice(start, end + endAnchor.length);
  return block.replaceAll(/!?phases\.includes\(/g, "PHASE_LIST.includes(");
}

test("the pin-registry setup/release block is the same protocol in every driver", () => {
  assertSharedAcrossAllDrivers(extractPinRegistryProtocol, "pin-registry block");
});

test("the report/acknowledgement/pin-set footer is the same protocol in every driver", () => {
  assertSharedAcrossAllDrivers(extractFooter, "footer");
});

test("pinKey, protocolVersion and pinProtocolVersion are declared identically between driver.js and shell_behaviour_driver.js; marker and stateKey are allowed to differ", () => {
  // extractPinRegistryProtocol's window starts at `let pinRegistry =
  // window[pinKey];`, so these five constants -- declared above that line
  // -- are read inside the extracted block but never themselves compared.
  // pinKey/protocolVersion/pinProtocolVersion are wire-protocol values two
  // separate WebView processes must still agree on; marker and stateKey
  // are deliberately per-run (RFC041_... vs RFC044_..., separate state
  // keys so the two runs never collide). A drift in the first three would
  // be silent forever: each driver is internally self-consistent, and
  // nothing else ever compares them against each other.
  //
  // trusted_click_driver.js is not part of this particular check (task 025
  // §2.5): its pinKey already differs from the other two on purpose (its
  // own WebView process never runs alongside either), a "constant" this
  // task's carve-out excuses -- the structural blocks above still cover it.
  for (const name of ["pinKey", "protocolVersion", "pinProtocolVersion"]) {
    assert.equal(
      declaredConstant(shellBehaviourJs, name, "shell_behaviour_driver.js"),
      declaredConstant(driverJs, name, "driver.js"),
      `${name} has drifted between driver.js and shell_behaviour_driver.js`,
    );
  }
  assert.notEqual(
    declaredConstant(shellBehaviourJs, "marker", "shell_behaviour_driver.js"),
    declaredConstant(driverJs, "marker", "driver.js"),
    "marker is meant to differ per run -- if it doesn't, that's worth knowing too",
  );
  assert.notEqual(
    declaredConstant(shellBehaviourJs, "stateKey", "shell_behaviour_driver.js"),
    declaredConstant(driverJs, "stateKey", "driver.js"),
    "stateKey is meant to differ per run -- if it doesn't, the two runs' state would collide",
  );
});

test("every driver's failEarly helper and request-shape check are the same protocol", () => {
  assertSharedAcrossAllDrivers(extractFailEarlyAndRequestCheck, "failEarly/request-check block");
});
