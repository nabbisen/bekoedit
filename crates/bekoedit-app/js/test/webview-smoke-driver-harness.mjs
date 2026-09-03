// Shared evaluator-pin driver harness for testing a `document::eval` driver
// script from a string, without a real WebView (task 012, consolidated per
// RFC-044 slice-1 handoff §8).
//
// `FakeDioxus`, `request`, `acknowledgement` and `runDriver` used to be
// duplicated between webview-smoke-driver.test.mjs and
// webview-smoke-driver-phases.test.mjs, and the two `request()` copies had
// already diverged -- one parameterised `phase`, the other hardcoded
// `"launch"`. This is the one copy both import from. `runDriver` takes the
// driver source as a parameter (rather than reading driver.js itself)
// because RFC-044 slice-1 adds a second driver with its own source file --
// the second consumer that made the drift real, and the reason this exists
// as its own module rather than being folded into webview-smoke-dom-fake.mjs
// (which is about DOM fidelity, a different concern from the channel
// protocol this harness fakes).

/** A fake `dioxus` channel: `recv`/`send`, matching what a
 * `document::eval` driver script receives as its function parameter. Queues
 * values pushed before a `recv()` call, and resolves a pending `recv()`
 * immediately when a value arrives. `close()` records that the Rust side
 * closed its query -- webview-smoke-driver.test.mjs uses this to prove the
 * evaluator pin survives it. */
export class FakeDioxus {
  constructor() {
    this.incoming = [];
    this.receivers = [];
    this.sent = [];
    this.sentWaiters = [];
    this.closed = false;
  }

  recv() {
    if (this.incoming.length > 0) return Promise.resolve(this.incoming.shift());
    return new Promise((resolve) => this.receivers.push(resolve));
  }

  send(value) {
    this.sent.push(value);
    this.sentWaiters.shift()?.(value);
  }

  push(value) {
    const receiver = this.receivers.shift();
    if (receiver) receiver(value);
    else this.incoming.push(value);
  }

  nextSent() {
    if (this.sent.length > 0) return Promise.resolve(this.sent.at(-1));
    return new Promise((resolve) => this.sentWaiters.push(resolve));
  }

  close() {
    this.closed = true;
  }
}

/** A `PhaseRequest`, matching what `run_driver_phase` sends -- the
 * transport is shared (RFC-044 slice-1 §3) so every driver reads the same
 * shape regardless of its own phase names. */
export function request(exchangeId, phase, release = null) {
  return {
    protocolVersion: 2,
    exchangeId,
    phase,
    releaseExchangeId: release?.exchangeId ?? null,
    releasePhase: release?.phase ?? null,
  };
}

/** A `PhaseAcknowledgement` echoing the fields `run_driver_phase` actually
 * echoes back, from a report the driver already sent. */
export function acknowledgement(report) {
  return {
    protocolVersion: report.protocolVersion,
    exchangeId: report.exchangeId,
    phase: report.phase,
    kind: report.kind,
  };
}

const AsyncFunction = async function () {}.constructor;

/** Evaluates `driverSource` (a driver script's text, exactly as
 * `document::eval` would run it) with `dioxus` as its channel parameter. */
export function runDriver(driverSource, dioxus) {
  return new AsyncFunction("dioxus", driverSource)(dioxus);
}
