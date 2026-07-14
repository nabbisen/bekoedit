import assert from "node:assert/strict";
import test from "node:test";

import { dispatchForRelayGeneration } from "../src/transport.js";

test("retired callable relay cannot dispatch before replacement acknowledgement", () => {
  const root = {};
  const dispatched = [];
  const retired = () => {};
  retired.__bkGeneration = 1;
  root.__relay = retired;

  assert.equal(
    dispatchForRelayGeneration(
      root,
      "__relay",
      2,
      (request) => dispatched.push(request),
      { type: "destroyEditor" },
    ),
    false,
  );
  assert.deepEqual(dispatched, []);

  const replacement = () => {};
  replacement.__bkGeneration = 2;
  root.__relay = replacement;
  assert.equal(
    dispatchForRelayGeneration(
      root,
      "__relay",
      2,
      (request) => dispatched.push(request),
      { type: "installRelay" },
    ),
    true,
  );
  assert.deepEqual(dispatched, [{ type: "installRelay" }]);
});
