// RFC-049 slice 2: the decision in assets/shortcuts.js for Form Mode's Undo and
// Redo. The script is an IIFE that installs a window keydown listener; it is
// run here in its own vm context with a fake window and document, so nothing
// leaks into the shared globals the other suites use.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("../../assets/shortcuts.js", import.meta.url), "utf8");

/** Loads shortcuts.js and returns `press(event, page)`, which fires one keydown
 * and returns `{ relayed, prevented }`. `page` is { formMode, active }. */
function load({ platform = "Linux x86_64" } = {}) {
  const listeners = {};
  const relayed = [];
  const page = { formMode: true, active: null };
  const window = {
    addEventListener: (type, fn) => (listeners[type] ??= []).push(fn),
    __bk_shortcut_relay: (message) => relayed.push(JSON.parse(message)),
  };
  const document = {
    querySelector: (selector) => (selector === ".form-mode" && page.formMode ? {} : null),
    get activeElement() {
      return page.active;
    },
  };
  vm.runInNewContext(source, { window, document, navigator: { platform }, Event: class {}, JSON });
  return {
    window,
    press(event, state = {}) {
      Object.assign(page, { formMode: true, active: null }, state);
      let prevented = false;
      relayed.length = 0;
      const e = {
        key: "z",
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
        altKey: false,
        isComposing: false,
        preventDefault: () => (prevented = true),
        ...event,
      };
      for (const fn of listeners.keydown) fn(e);
      return { relayed: relayed.map((m) => m.key), prevented };
    },
  };
}

const el = (tagName, extra = {}) => ({ tagName, isContentEditable: false, ...extra });
const CTRL_Z = { key: "z", ctrlKey: true };

test("Ctrl+Z with focus on body undoes (Form Mode shown, nothing focused)", () => {
  const { press } = load();
  assert.deepEqual(press(CTRL_Z, { active: el("BODY") }), {
    relayed: ["form_undo"],
    prevented: true,
  });
});

test("Ctrl+Z with no active element at all still undoes", () => {
  assert.deepEqual(load().press(CTRL_Z, { active: null }).relayed, ["form_undo"]);
});

test("Ctrl+Z with focus on a button undoes", () => {
  assert.deepEqual(load().press(CTRL_Z, { active: el("BUTTON") }).relayed, ["form_undo"]);
});

test("Ctrl+Z with focus on a select undoes", () => {
  assert.deepEqual(load().press(CTRL_Z, { active: el("SELECT") }).relayed, ["form_undo"]);
});

test("Ctrl+Z in an input is left to the browser", () => {
  assert.deepEqual(load().press(CTRL_Z, { active: el("INPUT") }), {
    relayed: [],
    prevented: false,
  });
});

test("Ctrl+Z in a textarea is left to the browser", () => {
  assert.deepEqual(load().press(CTRL_Z, { active: el("TEXTAREA") }), {
    relayed: [],
    prevented: false,
  });
});

test("Ctrl+Z in a contenteditable is left to the browser", () => {
  const editable = el("DIV", { isContentEditable: true });
  assert.deepEqual(load().press(CTRL_Z, { active: editable }).relayed, []);
  const inside = el("SPAN", { closest: (s) => (s === "[contenteditable]" ? {} : null) });
  assert.deepEqual(load().press(CTRL_Z, { active: inside }).relayed, []);
});

test("outside Form Mode (Text Mode, Preview) nothing is relayed or prevented", () => {
  assert.deepEqual(load().press(CTRL_Z, { formMode: false, active: el("BODY") }), {
    relayed: [],
    prevented: false,
  });
});

test("an open IME composition refuses the shortcut", () => {
  const { press, window } = load();
  assert.deepEqual(press({ ...CTRL_Z, isComposing: true }, { active: el("BODY") }).relayed, []);
  window.__bk_form_composing = true;
  assert.deepEqual(press(CTRL_Z, { active: el("BODY") }).relayed, []);
});

test("Ctrl+Shift+Z and Ctrl+Y redo, including a capital Z", () => {
  const { press } = load();
  assert.deepEqual(press({ key: "Z", ctrlKey: true, shiftKey: true }, { active: el("BODY") }).relayed, [
    "form_redo",
  ]);
  assert.deepEqual(press({ key: "z", ctrlKey: true, shiftKey: true }, { active: el("BODY") }).relayed, [
    "form_redo",
  ]);
  assert.deepEqual(press({ key: "y", ctrlKey: true }, { active: el("BODY") }).relayed, ["form_redo"]);
});

test("Ctrl+Y and Ctrl+Shift+Z in a field are left to the browser too", () => {
  const { press } = load();
  assert.deepEqual(press({ key: "y", ctrlKey: true }, { active: el("INPUT") }).relayed, []);
  assert.deepEqual(
    press({ key: "Z", ctrlKey: true, shiftKey: true }, { active: el("TEXTAREA") }).relayed,
    [],
  );
});

test("on macOS Cmd+Z and Cmd+Shift+Z are the shortcuts, and Ctrl+Z is not", () => {
  const { press } = load({ platform: "MacIntel" });
  assert.deepEqual(press({ key: "z", metaKey: true }, { active: el("BODY") }).relayed, ["form_undo"]);
  assert.deepEqual(
    press({ key: "z", metaKey: true, shiftKey: true }, { active: el("BODY") }).relayed,
    ["form_redo"],
  );
  assert.deepEqual(press(CTRL_Z, { active: el("BODY") }).relayed, []);
});

test("a bare Z, Ctrl+Alt+Z and Ctrl+Shift+Y are not shortcuts", () => {
  const { press } = load();
  assert.deepEqual(press({ key: "z" }, { active: el("BODY") }).relayed, []);
  assert.deepEqual(press({ key: "z", ctrlKey: true, altKey: true }, { active: el("BODY") }).relayed, []);
  assert.deepEqual(
    press({ key: "y", ctrlKey: true, shiftKey: true }, { active: el("BODY") }).relayed,
    [],
  );
});

test("the existing shortcuts still relay: Ctrl+S, Ctrl+2", () => {
  const { press } = load();
  assert.deepEqual(press({ key: "s", ctrlKey: true }, { active: el("BODY") }).relayed, ["save"]);
  assert.deepEqual(press({ key: "2", ctrlKey: true }, { active: el("BODY") }).relayed, ["mode_form"]);
});
