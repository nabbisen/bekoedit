/** RFC-046: the paste path. Converts a `paste` DOM event carrying HTML into a
 * `pasteRequested` bridge event, and applies the `pasteResult` reply CodeMirror
 * receives back, mapping the recorded selection through any edits that landed
 * while the conversion was in flight. Kept independent of RFC-041's lifecycle
 * state machine (`lifecycle.js`): a paste is not a mount/snapshot/resume
 * transition, and its result reaches the document as an ordinary transaction,
 * which the editor's own change pipeline then reports like any keystroke.
 */

import { BRIDGE_SCHEMA_VERSION } from "./lifecycle.js";

/** A JS string's `.length` counts UTF-16 code units, always `<=` its UTF-8
 * byte count: this is a conservative, one-directional "definitely too large"
 * check, matching `bekoedit_paste::MAX_HTML_BYTES` (1 MiB). The crate's own
 * byte-accurate check remains the authority for anything this admits. */
export const MAX_HTML_UTF16_LENGTH = 1024 * 1024;

const sameIdentity = (left, right) => Boolean(left && right)
  && left.instanceId === right.instanceId
  && left.editorId === right.editorId
  && left.documentId === right.documentId
  && left.epoch === right.epoch;

/** The text a reply's outcome inserts. Converted markdown loses its trailing
 * line break (`mdka` always ends output with one, §3.3): a pasted paragraph
 * should end where the pasted text ends, like a plain paste. Every other
 * outcome inserts the plain flavour cached at request time (`Outcome`'s own
 * doc comment: `Fallback` and `Empty` both mean "insert the plain text"). */
export function textForOutcome(outcome, plainText) {
  if (outcome && outcome.kind === "converted") {
    return outcome.markdown.replace(/[\r\n]+$/, "");
  }
  return plainText;
}

export function createPasteController(deps) {
  let plainPasteArmed = false;
  let pending = null;
  let nextToken = 1;

  function armPlainPaste() {
    plainPasteArmed = true;
  }

  /** Ctrl+Shift+V (Cmd+Shift+V on macOS). Kept even though a real
   * Ctrl+Shift+V already carries no `text/html` on Linux (RFC-046 §3.0): the
   * flag makes the behaviour the same on platforms that do not strip it. */
  function handleKeydown(event) {
    const isShiftV = event.shiftKey
      && (event.key === "v" || event.key === "V" || event.code === "KeyV");
    const isModified = event.ctrlKey || event.metaKey;
    if (isShiftV && isModified) armPlainPaste();
  }

  /** Returns `true` when the event was claimed (and `preventDefault`'d),
   * `false` to let CodeMirror's own plain paste run unmodified. */
  function handlePaste(event, view) {
    const wasArmed = plainPasteArmed;
    plainPasteArmed = false;
    if (wasArmed) return false;
    const clipboardData = event.clipboardData;
    const html = clipboardData?.getData?.("text/html");
    if (!html) return false;
    const plainText = clipboardData?.getData?.("text/plain") ?? "";
    event.preventDefault();
    const identity = deps.getIdentity();
    const { from, to } = deps.getSelection(view);
    const token = nextToken;
    nextToken += 1;
    if (html.length > MAX_HTML_UTF16_LENGTH) {
      deps.insert(view, { from, to, text: plainText });
      deps.emit({
        type: "pasteRequested",
        protocolVersion: BRIDGE_SCHEMA_VERSION,
        identity,
        token,
        html: null,
        plainLength: plainText.length,
      });
      return true;
    }
    pending = { token, identity, from, to, plainText };
    deps.emit({
      type: "pasteRequested",
      protocolVersion: BRIDGE_SCHEMA_VERSION,
      identity,
      token,
      html,
      plainLength: plainText.length,
    });
    return true;
  }

  /** Feed every document-changing update here (`EditorView.updateListener`),
   * so a pending paste's recorded selection tracks edits made while its
   * conversion is still in flight (§3.3). */
  function handleTransaction(update) {
    if (!pending || !update.docChanged) return;
    pending.from = update.changes.mapPos(pending.from, -1);
    pending.to = update.changes.mapPos(pending.to, 1);
  }

  /** Returns `true` when the message was a `pasteResult` this controller
   * handled (whether applied or discarded); `false` for anything else, so
   * the caller can fall through to its own dispatch. */
  function handleReply(message, view) {
    if (!message || message.type !== "pasteResult") return false;
    if (!pending || pending.token !== message.token) return true;
    const { from, to, identity, plainText } = pending;
    pending = null;
    if (deps.isHeld() || !sameIdentity(identity, deps.getIdentity())) {
      deps.emit({
        type: "pasteDiscarded",
        protocolVersion: BRIDGE_SCHEMA_VERSION,
        identity,
        token: message.token,
      });
      return true;
    }
    const text = textForOutcome(message.outcome, plainText);
    deps.insert(view, { from, to, text });
    return true;
  }

  return { handleKeydown, handlePaste, handleTransaction, handleReply };
}
