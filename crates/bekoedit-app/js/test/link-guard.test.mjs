// link_guard.js (task 032): the interception point, on a small DOM fake.
//
// The fake models what matters: `window` capture listeners run first, then
// the "interpreter" (a bubble listener on the inner root, standing for
// Dioxus's `handleClickNavigate`), and `stopPropagation` in the capture phase
// means the bubble listener never runs. The interpreter records a `browser_open`
// the way the real one sends it, so "a click on a relative link does not
// produce a `browser_open` for it" is asserted on the same ordering.
//
// Each test was checked against a mutated copy of the script (drop
// `stopPropagation`, register in the bubble phase, drop the anchor check,
// send on `auxclick`) and confirmed to fail -- see the review request.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(
  new URL("../../src/link_guard.js", import.meta.url),
  "utf8",
);

/** A listener list with a capture and a bubble side. */
class FakeTarget {
  constructor() {
    this.listeners = [];
  }
  addEventListener(type, handler, capture = false) {
    this.listeners.push({ type, handler, capture: capture === true });
  }
  removeEventListener(type, handler, capture = false) {
    this.listeners = this.listeners.filter(
      (l) => !(l.type === type && l.handler === handler && l.capture === (capture === true)),
    );
  }
}

/** `window` above `root`, as in the page: Dioxus's interpreter listens on
 * `root` (bubble phase), so a listener on `window` runs before it only if it
 * is a capture listener. Order: window capture, root capture, root bubble,
 * window bubble. Stopping propagation ends the rest. */
class FakeWindow extends FakeTarget {
  constructor() {
    super();
    this.root = new FakeTarget();
  }
  dispatch(event) {
    let stopped = false;
    event.stopPropagation = () => {
      stopped = true;
      event.propagationStopped = true;
    };
    event.preventDefault = () => {
      event.defaultPrevented = true;
    };
    const steps = [
      [this, true],
      [this.root, true],
      [this.root, false],
      [this, false],
    ];
    for (const [target, phase] of steps) {
      for (const l of [...target.listeners]) {
        if (stopped) return event;
        if (l.type === event.type && l.capture === phase) l.handler(event);
      }
    }
    return event;
  }
}

class FakeElement {
  constructor(tag, attributes = {}, parent = null) {
    this.tag = tag;
    this.attributes = attributes;
    this.parent = parent;
  }
  closest(selector) {
    for (let node = this; node; node = node.parent) {
      if (node.tag === selector) return node;
    }
    return null;
  }
  getAttribute(name) {
    return name in this.attributes ? this.attributes[name] : null;
  }
}

/** Runs the script the way `document::eval` does: `window` and `dioxus` in
 * scope, inside an async function. It never settles; that is intended. */
function install(win, dioxus) {
  if (!("interpreter" in win)) {
    win.interpreter = { intercept_link_redirects: true };
  }
  const run = new Function("window", "dioxus", `return (async () => {${source}})();`);
  run(win, dioxus);
}

function setup() {
  const win = new FakeWindow();
  const sent = [];
  const traces = [];
  const messages = [];
  const browserOpens = [];
  install(win, {
    send: (message) => {
      messages.push(message);
      if (message.kind === "click") sent.push(message.href);
      else traces.push(message.detail);
    },
  });
  // Dioxus's interpreter, as read in dioxus-interpreter-js 0.7.9
  // `handleClickNavigate`: a bubble-phase click listener that cancels the
  // click and sends the raw href of the enclosing <a> for `browser_open`.
  win.root.addEventListener("click", (event) => {
    if (!win.interpreter.intercept_link_redirects) return;
    const anchor = event.target?.closest?.("a");
    if (anchor) {
      event.preventDefault();
      const href = anchor.getAttribute("href");
      if (href) browserOpens.push(href);
    }
  });
  return { win, sent, traces, messages, browserOpens };
}

const anchor = (href) => new FakeElement("a", href === undefined ? {} : { href });

test("it listens for click and auxclick in the capture phase, on window", () => {
  const { win } = setup();
  const guard = win.listeners.filter((l) => l.capture);
  assert.deepEqual(guard.map((l) => l.type).sort(), ["auxclick", "click"]);
});

test("a click on a relative link is not sent to the interpreter's browser_open", () => {
  const { win, sent, browserOpens } = setup();
  const event = win.dispatch({ type: "click", target: anchor("other.md") });
  assert.deepEqual(browserOpens, [], "the interpreter never saw the click");
  assert.equal(event.defaultPrevented, true);
  assert.equal(event.propagationStopped, true);
  assert.deepEqual(sent, ["other.md"], "Rust gets the raw href, and decides");
});

test("no link click reaches the interpreter, whatever its href", () => {
  const { win, sent, browserOpens } = setup();
  const hrefs = ["#top", "../x.md", "https://example.com", "mailto:a@b.c", "//host/share", "C:\\a"];
  for (const href of hrefs) win.dispatch({ type: "click", target: anchor(href) });
  assert.deepEqual(browserOpens, []);
  assert.deepEqual(sent, hrefs);
});

test("a click on an element inside a link counts as a click on the link", () => {
  const { win, sent, browserOpens } = setup();
  const a = anchor("b.md");
  const emphasis = new FakeElement("em", {}, a);
  const text = new FakeElement("strong", {}, emphasis);
  win.dispatch({ type: "click", target: text });
  assert.deepEqual(sent, ["b.md"]);
  assert.deepEqual(browserOpens, []);
});

test("a link with no href sends the empty string, which Rust refuses", () => {
  const { win, sent } = setup();
  win.dispatch({ type: "click", target: anchor(undefined) });
  assert.deepEqual(sent, [""]);
});

test("a click that is not on a link is left alone", () => {
  const { win, sent, browserOpens } = setup();
  const button = new FakeElement("button");
  const event = win.dispatch({ type: "click", target: button });
  assert.equal(event.defaultPrevented, undefined);
  assert.equal(event.propagationStopped, undefined);
  assert.deepEqual(sent, []);
  assert.deepEqual(browserOpens, []);
  const textNode = {}; // no `closest`, as a text node has none
  assert.doesNotThrow(() => win.dispatch({ type: "click", target: textNode }));
  assert.doesNotThrow(() => win.dispatch({ type: "click", target: null }));
});

test("a middle click on a link is cancelled and sends nothing", () => {
  const { win, sent } = setup();
  const event = win.dispatch({ type: "auxclick", target: anchor("https://example.com") });
  assert.equal(event.defaultPrevented, true);
  assert.equal(event.propagationStopped, true);
  assert.deepEqual(sent, []);
});

test("installing again replaces the guard instead of doubling it", () => {
  const win = new FakeWindow();
  const first = [];
  const second = [];
  install(win, { send: (message) => first.push(message.href) });
  install(win, { send: (message) => second.push(message.href) });
  assert.equal(win.listeners.filter((l) => l.type === "click").length, 1);
  win.dispatch({ type: "click", target: anchor("a.md") });
  assert.deepEqual(first, []);
  assert.deepEqual(second, ["a.md"]);
});

test("a click is reported as { kind: 'click', href }, and nothing else is sent", () => {
  const { win, messages } = setup();
  win.dispatch({ type: "click", target: anchor("other.md") });
  assert.deepEqual(messages, [{ kind: "click", href: "other.md" }]);
});

test("installing switches the interpreter's own link route off", () => {
  const { win, traces } = setup();
  assert.equal(win.interpreter.intercept_link_redirects, false);
  assert.deepEqual(traces, [], "nothing to report when it worked");
});

test("with the guard's listener gone, a click still produces no browser_open", () => {
  const { win, browserOpens } = setup();
  // What if the capture-phase ordering differs on some WebView: the
  // interpreter's route is off, so it does nothing.
  win.removeEventListener("click", win.__bk_link_guard, true);
  win.dispatch({ type: "click", target: anchor("other.md") });
  assert.deepEqual(browserOpens, []);
  // Control: the same click does reach the interpreter if its route is on,
  // so the assertion above is able to see a leak.
  win.interpreter.intercept_link_redirects = true;
  win.dispatch({ type: "click", target: anchor("other.md") });
  assert.deepEqual(browserOpens, ["other.md"]);
});

test("a missing interpreter or a changed field is reported, not silent", () => {
  for (const interpreter of [undefined, {}, { intercept_link_redirects: "yes" }]) {
    const win = new FakeWindow();
    win.interpreter = interpreter;
    const messages = [];
    install(win, { send: (message) => messages.push(message) });
    assert.equal(messages.length, 1, JSON.stringify(interpreter));
    assert.equal(messages[0].kind, "trace");
    assert.match(messages[0].detail, /link route/);
    // Layer 1 still holds.
    assert.equal(win.listeners.filter((l) => l.capture).length, 2);
    if (interpreter && "intercept_link_redirects" in interpreter) {
      assert.equal(interpreter.intercept_link_redirects, "yes", "left as found");
    }
  }
});
