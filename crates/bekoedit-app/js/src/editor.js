/**
 * bekoedit CodeMirror 6 adapter (RFC-011).
 *
 * Exposes window.__bk:
 *   init(containerId, text, docId, revision, editorId, relayName, epoch)
 *   setDoc(text, docId, revision, epoch)      — external content update
 *   requestSnapshot(requestId, editorId, docId, epoch)
 *   focus()                                   — programmatic focus
 *
 * Sends to the active relay via window[relayName](JSON):
 *   {type:"change", editorId, docId, epoch, seq, text, composing}
 *   {type:"ready"}                            — editor mounted and ready
 *   {type:"scrollFraction", fraction}         — scroll position for Split sync
 *
 * IME composition safety (RFC-011 / MVP checklist):
 *   compositionstart → compositionActive = true; cancel pending timer
 *   compositionend   → compositionActive = false; flush immediately
 *   updateListener   → skips scheduling while compositionActive is true
 *
 * The Rust side owns authoritative revisions; JS revision fields are the
 * base the snapshot was taken from (RFC-011 snapshot strategy).
 */

import { EditorView, keymap, placeholder, drawSelection } from "@codemirror/view";
import { EditorState, Transaction } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { search, searchKeymap } from "@codemirror/search";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  indentOnInput,
} from "@codemirror/language";

// --- Editor state -----------------------------------------------------------

let view = null;
let currentDocId = null;
let currentEditorId = "text";
let currentRelayName = "__bk_text_relay";
let currentEpoch = 0;
let seq = 0;
let sendTimer = null;
let skipNextSend = false;   // suppress send when Rust pushes setDoc
let compositionActive = false; // true during CJK / IME composition

function sendToRust(payload) {
  window[currentRelayName]?.(JSON.stringify(payload));
}

function sendChange() {
  if (!view || skipNextSend) return;
  const text = view.state.doc.toString();
  seq += 1;
  sendToRust({
    type: "change",
    editorId: currentEditorId,
    docId: currentDocId,
    epoch: currentEpoch,
    seq,
    text,
    composing: false,
  });
}

function scheduleSend(immediate) {
  clearTimeout(sendTimer);
  // Never send during active IME composition — wait for compositionend.
  if (compositionActive) return;
  sendTimer = setTimeout(sendChange, immediate ? 0 : 100);
}

// --- Theme ------------------------------------------------------------------

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

// --- Extensions -------------------------------------------------------------

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
    // Suppress sends during CJK/IME composition (RFC-011 IME safety).
    EditorView.domEventHandlers({
      compositionstart() {
        compositionActive = true;
        clearTimeout(sendTimer);
      },
      compositionend() {
        compositionActive = false;
        // Flush committed text immediately after composition finishes.
        scheduleSend(true);
      },
    }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        scheduleSend(false);
      }
    }),
    // Scroll-fraction reporter for Split Mode sync (RFC-012).
    EditorView.domEventHandlers({
      scroll(evt, view) {
        const scroller = view.scrollDOM;
        const max = scroller.scrollHeight - scroller.clientHeight;
        if (max > 0) {
          const fraction = scroller.scrollTop / max;
          sendToRust({ type: "scroll", fraction });
        }
      },
    }),
  ];
}

// --- Public API -------------------------------------------------------------

function init(containerId, text, docId, revision, editorId = "text", relayName = "__bk_text_relay", epoch = 0) {
  currentDocId = docId;
  currentEditorId = editorId;
  currentRelayName = relayName;
  currentEpoch = epoch;
  seq = 0;
  compositionActive = false;

  const parent = document.getElementById(containerId);
  if (!parent) {
    console.warn("bekoedit: container not found:", containerId);
    return;
  }

  if (view) { view.destroy(); view = null; }

  view = new EditorView({
    state: EditorState.create({ doc: text, extensions: buildExtensions() }),
    parent,
  });

  sendToRust({ type: "ready", editorId: currentEditorId, docId: currentDocId, epoch: currentEpoch });
}

function setDoc(text, docId, revision, epoch = currentEpoch) {
  if (!view) return;

  const isSameDoc = docId === currentDocId;
  currentDocId = docId;
  currentEpoch = epoch;
  seq = 0;
  compositionActive = false; // external update always clears composition state

  skipNextSend = true;
  const current = view.state.doc.toString();
  if (current !== text) {
    if (isSameDoc) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        annotations: Transaction.userEvent.of("remote"),
      });
    } else {
      view.setState(EditorState.create({ doc: text, extensions: buildExtensions() }));
    }
  }
  Promise.resolve().then(() => { skipNextSend = false; });
}

function requestSnapshot(requestId, editorId, docId, epoch) {
  if (!view) {
    sendToRust({ type: "snapshotBlocked", requestId, editorId, docId, epoch, reason: "editorUnavailable" });
    return;
  }
  if (editorId !== currentEditorId || docId !== currentDocId || epoch !== currentEpoch) {
    sendToRust({ type: "snapshotBlocked", requestId, editorId, docId, epoch, reason: "identityMismatch" });
    return;
  }
  if (compositionActive) {
    sendToRust({ type: "snapshotBlocked", requestId, editorId, docId, epoch, reason: "compositionActive" });
    return;
  }
  clearTimeout(sendTimer);
  seq += 1;
  sendToRust({
    type: "snapshot",
    requestId,
    editorId,
    docId,
    epoch,
    seq,
    text: view.state.doc.toString(),
    composing: false,
  });
}

function focus() { view?.focus(); }

function undo() {
    if (!view) return;
    import("@codemirror/commands").then(({ undo: _undo }) => _undo(view));
}

function redo() {
    if (!view) return;
    import("@codemirror/commands").then(({ redo: _redo }) => _redo(view));
}

window.__bk = { init, setDoc, requestSnapshot, focus, undo, redo, get _view() { return view; } };
export { init, setDoc, requestSnapshot, focus, undo, redo };
