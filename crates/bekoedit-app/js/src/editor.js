/**
 * bekoedit CodeMirror 6 adapter (RFC-011).
 *
 * Exposes window.__bk:
 *   init(containerId, text, docId, revision)  — create/replace the editor
 *   setDoc(text, docId, revision)             — external content update
 *   focus()                                   — programmatic focus
 *
 * Sends to Dioxus via window.__bk_relay(JSON):
 *   {type:"change", docId, revision, text}    — user edits (100 ms debounce,
 *                                               suppressed during IME composition)
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
let currentRevision = 0;
let sendTimer = null;
let skipNextSend = false;   // suppress send when Rust pushes setDoc
let compositionActive = false; // true during CJK / IME composition

function sendChange() {
  if (!view || skipNextSend) return;
  const text = view.state.doc.toString();
  window.__bk_relay?.(
    JSON.stringify({ type: "change", docId: currentDocId, revision: currentRevision, text })
  );
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
          window.__bk_relay?.(
            JSON.stringify({ type: "scrollFraction", fraction })
          );
        }
      },
    }),
  ];
}

// --- Public API -------------------------------------------------------------

function init(containerId, text, docId, revision) {
  currentDocId = docId;
  currentRevision = revision;
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

  window.__bk_relay?.(JSON.stringify({ type: "ready" }));
}

function setDoc(text, docId, revision) {
  currentRevision = revision;
  if (!view) return;

  const isSameDoc = docId === currentDocId;
  currentDocId = docId;
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

function focus() { view?.focus(); }

window.__bk = { init, setDoc, focus, get _view() { return view; } };
export { init, setDoc, focus };
