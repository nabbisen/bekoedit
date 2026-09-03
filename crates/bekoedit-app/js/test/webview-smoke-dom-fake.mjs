// Controllable DOM fake for driver.js's phase/DOM half (task 012).
//
// driver.js (src/webview_smoke/driver.js) is delivered by `include_str!`
// and evaluated from a string in webview-smoke-driver.test.mjs, so no
// coverage tool can see it. This fake exists to make its phase machine
// testable without a real browser, `jsdom`, or `xvfb` -- see the review
// request for how coverage was measured instead (mutation, not
// tooling).
//
// Fidelity is bounded by what driver.js actually calls (verified by
// reading the file, not guessed): `document.querySelector`,
// `element.dispatchEvent`, `element.matches`/`element.querySelector`
// (inside the error-toast check), `MutationObserver.observe`/
// `.disconnect()`, `performance.now`, `window.__bk?._view` with
// `.dispatch`/`.state.doc.length`/`.dom.isConnected`/`.hasFocus`, and
// `article.textContent`. Nothing beyond that list is implemented here on
// purpose (task 012 §3.4) -- this is not a DOM, it is a lookup table
// shaped like the handful of calls one file makes.

/** A fake element: dispatchEvent, matches, querySelector, textContent --
 * exactly what driver.js reads or calls on a DOM node. */
export class FakeElement {
  constructor({
    nodeType = 1, // Node.ELEMENT_NODE
    dispatchResult = true,
    textContent = "",
  } = {}) {
    this.nodeType = nodeType;
    this.dispatchResult = dispatchResult;
    this.textContent = textContent;
    this.dispatchedEvents = [];
    this._matchSelectors = new Set();
    this._children = new Map();
  }

  /** Makes `element.matches(selector)` return true for this selector. */
  matching(selector) {
    this._matchSelectors.add(selector);
    return this;
  }

  /** Makes `element.querySelector(selector)` return `child`. */
  withChild(selector, child) {
    this._children.set(selector, child);
    return this;
  }

  dispatchEvent(event) {
    this.dispatchedEvents.push(event);
    return this.dispatchResult;
  }

  matches(selector) {
    return this._matchSelectors.has(selector);
  }

  querySelector(selector) {
    return this._children.get(selector) ?? null;
  }
}

/** A fake CodeMirror view, matching only the shape `window.__bk._view`
 * needs: `.dom.isConnected`, `.hasFocus`, `.state.doc.length`,
 * `.dispatch(patch)`. */
export class FakeEditorView {
  constructor({ connected = true, hasFocus = true, docLength = 0 } = {}) {
    this.dom = { isConnected: connected };
    this.hasFocus = hasFocus;
    this.state = { doc: { length: docLength } };
    this.dispatchCalls = [];
  }

  dispatch(patch) {
    this.dispatchCalls.push(patch);
  }
}

/** Installs a minimal global environment shaped like the calls driver.js
 * makes, and gives the test full control over every one of them:
 * `setElement`/selectors resolved by `document.querySelector`, `setTime`
 * for `performance.now()`, `setEditorView` for `window.__bk._view`, and
 * `emitMutation` to drive the installed `MutationObserver` callback
 * directly (no real DOM mutation ever happens; the callback is called by
 * hand with synthetic records, exactly as the browser would call it). */
export class FakeDom {
  constructor() {
    this.elements = new Map();
    this.time = 0;
    this.observerCallback = null;
    this.observerConnected = false;
  }

  install() {
    globalThis.window = { __bk: undefined };
    globalThis.Node = { ELEMENT_NODE: 1 };
    globalThis.MouseEvent = class MouseEvent {
      constructor(type, init) {
        this.type = type;
        Object.assign(this, init);
      }
    };
    globalThis.performance = { now: () => this.time };
    const self = this;
    globalThis.MutationObserver = class MutationObserver {
      constructor(callback) {
        self.observerCallback = callback;
      }
      observe() {
        self.observerConnected = true;
      }
      disconnect() {
        self.observerConnected = false;
      }
    };
    globalThis.document = {
      documentElement: {},
      querySelector: (selector) => this.elements.get(selector) ?? null,
    };
  }

  /** `document.querySelector(selector)` will return `element` (or
   * `null`, the default, if never set). */
  setElement(selector, element) {
    this.elements.set(selector, element ?? null);
    return this;
  }

  /** `performance.now()` will return exactly this value until changed
   * again -- no real clock, per task 012 §3.5. */
  setTime(ms) {
    this.time = ms;
  }

  /** `window.__bk._view` will be `view` (or `undefined` to simulate the
   * editor not having mounted yet). */
  setEditorView(view) {
    window.__bk = view === undefined ? undefined : { _view: view };
  }

  /** Calls the driver's installed `MutationObserver` callback directly
   * with one synthetic record. Fails loudly if no observer has been
   * installed yet, rather than silently doing nothing. */
  emitMutation({ addedNodes = [], target = null } = {}) {
    if (!this.observerCallback) {
      throw new Error("emitMutation: no MutationObserver has been installed");
    }
    this.observerCallback([{ addedNodes, target }]);
  }
}

/** A `.toast-error` element, ready to pass to `setElement` (for the
 * initial `errorToastSeen` check) or `emitMutation` (for one observed
 * mid-run). */
export function errorToastElement() {
  return new FakeElement().matching(".toast-error");
}
