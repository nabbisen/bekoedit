import assert from "node:assert/strict";
import test from "node:test";

import {
  MAX_HTML_UTF16_LENGTH,
  createPasteController,
  textForOutcome,
} from "../src/paste.js";

const identity = (overrides = {}) => ({
  instanceId: 1,
  editorId: "text",
  documentId: 7,
  epoch: 1,
  ...overrides,
});

function clipboardEvent({ html, plain } = {}) {
  let prevented = false;
  return {
    preventDefault() { prevented = true; },
    get defaultPrevented() { return prevented; },
    clipboardData: {
      getData(kind) {
        if (kind === "text/html") return html ?? "";
        if (kind === "text/plain") return plain ?? "";
        return "";
      },
    },
  };
}

function keydownEvent({ key = "v", shiftKey = true, ctrlKey = true, metaKey = false } = {}) {
  return { key, code: "KeyV", shiftKey, ctrlKey, metaKey };
}

function harness({ currentIdentity = identity(), held = false } = {}) {
  const emitted = [];
  const inserted = [];
  let liveIdentity = currentIdentity;
  let isHeld = held;
  const controller = createPasteController({
    emit(payload) { emitted.push(payload); },
    getIdentity: () => liveIdentity,
    isHeld: () => isHeld,
    getSelection: (view) => ({ from: view.from, to: view.to }),
    insert(_view, change) { inserted.push(change); },
  });
  return {
    controller,
    emitted,
    inserted,
    setIdentity: (next) => { liveIdentity = next; },
    setHeld: (next) => { isHeld = next; },
  };
}

const view = (from = 3, to = 3) => ({ from, to });

test("a paste with no HTML flavour is left for CodeMirror's own plain paste", () => {
  const { controller, emitted, inserted } = harness();
  const event = clipboardEvent({ plain: "just text" });
  const claimed = controller.handlePaste(event, view());
  assert.equal(claimed, false);
  assert.equal(event.defaultPrevented, false);
  assert.deepEqual(emitted, []);
  assert.deepEqual(inserted, []);
});

test("a paste with HTML is claimed, prevented, and requested over the bridge", () => {
  const { controller, emitted } = harness();
  const event = clipboardEvent({ html: "<p>hi</p>", plain: "hi" });
  const claimed = controller.handlePaste(event, view(3, 5));
  assert.equal(claimed, true);
  assert.equal(event.defaultPrevented, true);
  assert.equal(emitted.length, 1);
  assert.equal(emitted[0].type, "pasteRequested");
  assert.equal(emitted[0].html, "<p>hi</p>");
  assert.equal(emitted[0].plainLength, 2);
  assert.equal(emitted[0].token, 1);
});

test("the plain-paste flag consumes exactly the next paste, HTML or not", () => {
  const { controller, emitted } = harness();
  controller.handleKeydown(keydownEvent());
  const armed = controller.handlePaste(clipboardEvent({ html: "<p>hi</p>" }), view());
  assert.equal(armed, false);
  assert.deepEqual(emitted, []);
  // The flag was one-shot: the next paste with HTML is claimed again.
  const next = controller.handlePaste(clipboardEvent({ html: "<p>hi</p>" }), view());
  assert.equal(next, true);
  assert.equal(emitted.length, 1);
});

test("a plain keydown without the modifiers does not arm the flag", () => {
  const { controller, emitted } = harness();
  controller.handleKeydown(keydownEvent({ ctrlKey: false, metaKey: false }));
  controller.handleKeydown({ key: "v", shiftKey: false, ctrlKey: true, metaKey: false });
  const claimed = controller.handlePaste(clipboardEvent({ html: "<p>hi</p>" }), view());
  assert.equal(claimed, true);
  assert.equal(emitted.length, 1);
});

test("an HTML flavour over the size limit is decided locally, without crossing the bridge", () => {
  const { controller, emitted, inserted } = harness();
  const oversized = "x".repeat(MAX_HTML_UTF16_LENGTH + 1);
  const event = clipboardEvent({ html: oversized, plain: "cached plain text" });
  const claimed = controller.handlePaste(event, view(4, 4));
  assert.equal(claimed, true);
  assert.equal(event.defaultPrevented, true);
  assert.deepEqual(inserted, [{ from: 4, to: 4, text: "cached plain text" }]);
  assert.equal(emitted.length, 1);
  assert.equal(emitted[0].html, null);
  assert.equal(emitted[0].plainLength, "cached plain text".length);
});

test("a converted reply inserts the markdown, with its trailing line break stripped", () => {
  const { controller, inserted } = harness();
  controller.handlePaste(clipboardEvent({ html: "<h1>Hi</h1>", plain: "Hi" }), view(2, 2));
  const handled = controller.handleReply(
    { type: "pasteResult", token: 1, outcome: { kind: "converted", markdown: "# Hi\n\n" } },
    view(),
  );
  assert.equal(handled, true);
  assert.deepEqual(inserted, [{ from: 2, to: 2, text: "# Hi" }]);
});

test("each fallback reply inserts the cached plain flavour", () => {
  for (const reason of ["tooLarge", "failed", "timedOut"]) {
    const { controller, inserted } = harness();
    controller.handlePaste(clipboardEvent({ html: "<p>hi</p>", plain: "hi" }), view(0, 0));
    controller.handleReply(
      { type: "pasteResult", token: 1, outcome: { kind: "fallback", reason } },
      view(),
    );
    assert.deepEqual(inserted, [{ from: 0, to: 0, text: "hi" }]);
  }
});

test("an empty reply inserts the cached plain flavour, silently", () => {
  const { controller, inserted } = harness();
  controller.handlePaste(clipboardEvent({ html: "<p></p>", plain: "" }), view(0, 0));
  controller.handleReply({ type: "pasteResult", token: 1, outcome: { kind: "empty" } }, view());
  assert.deepEqual(inserted, [{ from: 0, to: 0, text: "" }]);
});

test("the recorded selection is mapped through edits made while conversion is in flight", () => {
  const { controller, inserted } = harness();
  controller.handlePaste(clipboardEvent({ html: "<p>hi</p>", plain: "hi" }), view(10, 10));
  // Five characters inserted at position 0 push the recorded position right.
  controller.handleTransaction({
    docChanged: true,
    changes: { mapPos: (pos) => pos + 5 },
  });
  controller.handleReply(
    { type: "pasteResult", token: 1, outcome: { kind: "converted", markdown: "x" } },
    view(),
  );
  assert.deepEqual(inserted, [{ from: 15, to: 15, text: "x" }]);
});

test("an update with no document change does not move the recorded selection", () => {
  const { controller, inserted } = harness();
  controller.handlePaste(clipboardEvent({ html: "<p>hi</p>", plain: "hi" }), view(10, 10));
  controller.handleTransaction({ docChanged: false, changes: { mapPos: () => 999 } });
  controller.handleReply(
    { type: "pasteResult", token: 1, outcome: { kind: "converted", markdown: "x" } },
    view(),
  );
  assert.deepEqual(inserted, [{ from: 10, to: 10, text: "x" }]);
});

test("a reply for the wrong identity is discarded, raising exactly one notice", () => {
  const { controller, inserted, emitted, setIdentity } = harness({ currentIdentity: identity() });
  controller.handlePaste(clipboardEvent({ html: "<p>hi</p>", plain: "hi" }), view());
  setIdentity(identity({ epoch: 2 }));
  const handled = controller.handleReply(
    { type: "pasteResult", token: 1, outcome: { kind: "converted", markdown: "x" } },
    view(),
  );
  assert.equal(handled, true);
  assert.deepEqual(inserted, []);
  const discards = emitted.filter((message) => message.type === "pasteDiscarded");
  assert.equal(discards.length, 1);
  assert.equal(discards[0].token, 1);
});

test("a reply that arrives while the editor is held is discarded, raising exactly one notice", () => {
  const { controller, inserted, emitted, setHeld } = harness();
  controller.handlePaste(clipboardEvent({ html: "<p>hi</p>", plain: "hi" }), view());
  setHeld(true);
  controller.handleReply(
    { type: "pasteResult", token: 1, outcome: { kind: "converted", markdown: "x" } },
    view(),
  );
  assert.deepEqual(inserted, []);
  assert.equal(emitted.filter((message) => message.type === "pasteDiscarded").length, 1);
});

test("a reply for a token that is not pending is swallowed without inserting or emitting", () => {
  const { controller, inserted, emitted } = harness();
  const handled = controller.handleReply(
    { type: "pasteResult", token: 999, outcome: { kind: "converted", markdown: "x" } },
    view(),
  );
  assert.equal(handled, true);
  assert.deepEqual(inserted, []);
  assert.deepEqual(emitted, []);
});

test("a non-pasteResult message is left for the caller's own dispatch", () => {
  const { controller } = harness();
  assert.equal(controller.handleReply({ type: "change" }, view()), false);
  assert.equal(controller.handleReply(null, view()), false);
});

test("textForOutcome strips only a converted outcome's trailing line breaks", () => {
  assert.equal(
    textForOutcome({ kind: "converted", markdown: "line one\nline two\n\n" }, "plain"),
    "line one\nline two",
  );
  assert.equal(textForOutcome({ kind: "fallback", reason: "failed" }, "plain text"), "plain text");
  assert.equal(textForOutcome({ kind: "empty" }, "plain text"), "plain text");
});
