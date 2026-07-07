/**
 * bekoedit CodeMirror 6 adapter (RFC-011).
 *
 * Exposes window.__bk:
 *   init(containerId, text, docId, revision)  — create/replace the editor
 *   setDoc(text, docId, revision)             — external content update
 *   focus()                                   — programmatic focus
 *
 * Sends to Dioxus via dioxus.send(JSON):
 *   {type:"change", docId, revision, text}    — user edits (100 ms debounce)
 *   {type:"ready"}                            — editor mounted and ready
 *
 * The Rust side owns authoritative revisions; JS revision fields are the
 * base the snapshot was taken from (RFC-011 snapshot strategy).
 */

import { EditorView, keymap, placeholder, drawSelection } from "@codemirror/view";
import { EditorState, Transaction } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { search, searchKeymap, openSearchPanel } from "@codemirror/search";
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
let skipNextSend = false; // suppress send when Rust pushes setDoc

function sendChange() {
  if (!view || skipNextSend) return;
  const text = view.state.doc.toString();
  if (window.dioxus) {
    window.__bk_relay?.(
      JSON.stringify({ type: "change", docId: currentDocId, revision: currentRevision, text })
    );
  }
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
  ".cm-gutters": { borderRight: "1px solid #ddd8cc", background: "#f8f7f3", color: "#8a857a" },
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
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        clearTimeout(sendTimer);
        sendTimer = setTimeout(sendChange, 100);
      }
    }),
  ];
}

// --- Public API -------------------------------------------------------------

/**
 * Mount (or remount) the editor into `containerId`.
 * Called on every document open / mode switch to Text.
 */
function init(containerId, text, docId, revision) {
  currentDocId = docId;
  currentRevision = revision;

  const parent = document.getElementById(containerId);
  if (!parent) {
    console.warn("bekoedit: container not found:", containerId);
    return;
  }

  if (view) {
    view.destroy();
    view = null;
  }

  view = new EditorView({
    state: EditorState.create({ doc: text, extensions: buildExtensions() }),
    parent,
  });

  if (window.dioxus) {
    window.__bk_relay?.(JSON.stringify({ type: "ready" }));
  }
}

/**
 * Update editor content when Rust changes the document externally
 * (reload from disk, recovery restore, mode switch back to Text).
 * Preserves cursor position if the document identity is the same.
 */
function setDoc(text, docId, revision) {
  currentRevision = revision;
  if (!view) return;

  const isSameDoc = docId === currentDocId;
  currentDocId = docId;

  skipNextSend = true;
  const current = view.state.doc.toString();
  if (current !== text) {
    if (isSameDoc) {
      // Minimal transaction: replace content, keep cursor within bounds.
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        annotations: Transaction.userEvent.of("remote"),
      });
    } else {
      // New document: full state replacement.
      view.setState(EditorState.create({ doc: text, extensions: buildExtensions() }));
    }
  }
  // Use a micro-task so the update listener fires first.
  Promise.resolve().then(() => { skipNextSend = false; });
}

function focus() {
  view?.focus();
}

// Attach to window so Rust's eval calls can reach it.
// Expose the EditorView instance for outline navigation (RFC-010).
Object.defineProperty(window.__bk || {}, '_view', { get: () => view });
window.__bk = { init, setDoc, focus, get _view() { return view; } };

export { init, setDoc, focus };
