/** CodeMirror 6 protocol-v2 adapter for RFC-041. */

import { EditorView, keymap, placeholder, drawSelection } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { search, searchKeymap } from "@codemirror/search";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  indentOnInput,
} from "@codemirror/language";

import { BRIDGE_SCHEMA_VERSION, createLifecycleAdapter } from "./lifecycle.js";
import { dispatchForRelayGeneration } from "./transport.js";

const RELAY_NAME = "__bk_source_editor_relay";
let view = null;
let sendTimer = null;
let skipNextSend = false;
let adapter = null;

function emit(payload) {
  const relay = window[RELAY_NAME];
  if (typeof relay !== "function") {
    console.warn("bekoedit: source editor relay unavailable", payload.type);
    return false;
  }
  relay(JSON.stringify(payload));
  return true;
}

function trace(name) {
  const identity = adapter?.currentIdentity();
  emit({
    type: "trace",
    protocolVersion: BRIDGE_SCHEMA_VERSION,
    instanceId: identity?.instanceId ?? null,
    event: name,
  });
}

function cancelPendingChange() {
  clearTimeout(sendTimer);
  sendTimer = null;
}

function sendChange() {
  sendTimer = null;
  if (!view || skipNextSend) return;
  adapter.publishChange(view.state.doc.toString());
}

function scheduleSend(immediate) {
  cancelPendingChange();
  if (!view || skipNextSend || adapter.isHeld()) return;
  sendTimer = setTimeout(sendChange, immediate ? 0 : 100);
}

const bekoeditTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "14px",
    fontFamily: "'SFMono-Regular', Consolas, 'Liberation Mono', monospace",
  },
  ".cm-scroller": { overflow: "auto", lineHeight: "1.6" },
  ".cm-content": { padding: "16px 20px", caretColor: "#2f6f5f" },
  ".cm-cursor": { borderLeftColor: "#2f6f5f" },
  ".cm-activeLine": { backgroundColor: "#f1efe940" },
  ".cm-selectionBackground, .cm-focused .cm-selectionBackground": {
    background: "#2f6f5f33",
  },
  ".cm-gutters": {
    borderRight: "1px solid #ddd8cc",
    background: "#f8f7f3",
    color: "#8a857a",
  },
  ".cm-lineNumbers": { minWidth: "36px" },
  ".cm-foldPlaceholder": { backgroundColor: "#e7e3d8" },
}, { dark: false });

function buildExtensions() {
  return [
    history(),
    drawSelection(),
    indentOnInput(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    search({ top: true }),
    EditorView.lineWrapping,
    placeholder("Start writing Markdown…"),
    keymap.of([
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
      indentWithTab,
    ]),
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    bekoeditTheme,
    EditorState.transactionFilter.of((transaction) => {
      if (transaction.docChanged && adapter?.isHeld()) return [];
      return transaction;
    }),
    EditorView.domEventHandlers({
      compositionstart() {
        adapter.compositionStarted();
      },
      compositionend() {
        adapter.compositionEnded();
        scheduleSend(true);
      },
      scroll(_event, currentView) {
        const scroller = currentView.scrollDOM;
        const max = scroller.scrollHeight - scroller.clientHeight;
        if (max > 0) emit({ type: "scroll", fraction: scroller.scrollTop / max });
      },
    }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged && !skipNextSend) scheduleSend(false);
    }),
  ];
}

adapter = createLifecycleAdapter({
  emit,
  getContainer: (containerId) => document.getElementById(containerId),
  createView(parent, text) {
    view = new EditorView({
      state: EditorState.create({ doc: text, extensions: buildExtensions() }),
      parent,
    });
    return view;
  },
  destroyView(currentView) {
    currentView.destroy();
    if (view === currentView) view = null;
  },
  getText: (currentView) => currentView.state.doc.toString(),
  replaceDocument(currentView, text) {
    skipNextSend = true;
    currentView.setState(EditorState.create({ doc: text, extensions: buildExtensions() }));
    queueMicrotask(() => { skipNextSend = false; });
  },
  focusView: (currentView) => currentView.focus(),
  cancelPendingChange,
});

function dispatch(request) {
  try {
    const parsed = typeof request === "string" ? JSON.parse(request) : request;
    return adapter.dispatch(parsed);
  } catch (_error) {
    trace("js.dispatch.bridge_error");
    return false;
  }
}

function dispatchForGeneration(request, generation) {
  return dispatchForRelayGeneration(
    window,
    RELAY_NAME,
    generation,
    dispatch,
    request,
  );
}

function focus() { adapter.focus(); }

function undo() {
  if (!view || adapter.isHeld()) return;
  import("@codemirror/commands").then(({ undo: runUndo }) => runUndo(view));
}

function redo() {
  if (!view || adapter.isHeld()) return;
  import("@codemirror/commands").then(({ redo: runRedo }) => runRedo(view));
}

window.__bk = {
  protocolVersion: BRIDGE_SCHEMA_VERSION,
  dispatch,
  dispatchForRelayGeneration: dispatchForGeneration,
  focus,
  undo,
  redo,
  get _view() { return view; },
};

export { BRIDGE_SCHEMA_VERSION, dispatch, focus, undo, redo };
