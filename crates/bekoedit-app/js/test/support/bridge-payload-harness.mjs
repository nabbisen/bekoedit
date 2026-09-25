// Task 031: evaluates the source Rust's `dispatch_request_js` emits, the way
// the page does, against a stub `window`, and checks what arrives.
//
// Run by `source_sync/host/tests.rs` (the emitted source only exists in Rust),
// not by `npm test`: `node bridge-payload-harness.mjs <mode> <script> <expected>
// <protocolVersion> <generation> <relayName>`. Exits non-zero, naming what
// differed, if the request that reaches the page is not deep-equal to the
// original, or did not arrive as a string for `JSON.parse`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const [mode, scriptPath, expectedPath, version, generation, relayName] = process.argv.slice(2);
const script = readFileSync(scriptPath, "utf8");
const expected = JSON.parse(readFileSync(expectedPath, "utf8"));

let arrivedAs = null;
let dispatched = null;
let relayed = null;
const window = {
  __bk: {
    protocolVersion: Number(version),
    dispatchForRelayGeneration(request, gen) {
      assert.equal(gen, Number(generation));
      if (mode === "fallback") return false;
      arrivedAs = typeof request;
      // The one-line branch `editor.js`'s `dispatch` has, unchanged.
      dispatched = typeof request === "string" ? JSON.parse(request) : request;
      return true;
    },
  },
};
if (mode === "fallback") {
  const relay = (message) => {
    relayed = message;
  };
  relay.__bkGeneration = Number(generation);
  window[relayName] = relay;
}
// No real waiting: the emitted loop retries up to 80 times, 50 ms apart.
const context = { window, setTimeout: (callback) => callback() };
await vm.runInNewContext(script, context);

if (mode === "dispatch") {
  assert.equal(arrivedAs, "string", "the request must reach the page as a string for JSON.parse");
  assert.deepStrictEqual(dispatched, expected);
} else {
  assert.equal(typeof relayed, "string", "the fallback must reach the relay as a string");
  assert.deepStrictEqual(JSON.parse(relayed), expected);
}
