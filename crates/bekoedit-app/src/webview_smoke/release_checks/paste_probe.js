// RFC-046 slice 2, part A: what does the WebView do with a paste? Defines
// window.__pasteProbe. Records; asserts nothing.
(() => {
  const state = { events: [], t0: 0, installed: false };
  const head = (text) => (typeof text === "string" ? text.slice(0, 80) : null);
  const view = () => (window.__bk && window.__bk._view) || null;
  const doc = () => (view() ? view().state.doc.toString() : null);

  const record = (type, event) => {
    const target = event.target;
    const rec = {
      kind: type,
      at: Math.round(performance.now() - state.t0),
      target: target ? String(target.className || target.tagName || "") : null,
      trusted: event.isTrusted === true,
      defaultPrevented: event.defaultPrevented === true,
    };
    if (type === "keydown" || type === "keyup") {
      rec.key = event.key;
      rec.code = event.code;
      rec.ctrl = event.ctrlKey;
      rec.shift = event.shiftKey;
      rec.meta = event.metaKey;
    }
    if (type === "beforeinput" || type === "input") {
      rec.inputType = event.inputType;
      rec.data = head(event.data);
    }
    if (type === "paste") {
      const data = event.clipboardData;
      rec.hasClipboardData = !!data;
      if (data) {
        rec.types = Array.from(data.types || []);
        const html = data.getData("text/html");
        const plain = data.getData("text/plain");
        rec.htmlLength = html.length;
        rec.htmlHead = head(html);
        rec.plainLength = plain.length;
        rec.plainHead = head(plain);
      }
    }
    state.events.push(rec);
  };

  const install = () => {
    if (state.installed) return true;
    state.t0 = performance.now();
    for (const type of ["keydown", "keyup", "beforeinput", "input", "paste"]) {
      document.addEventListener(type, (event) => record(type, event), true);
    }
    state.installed = true;
    return true;
  };

  // The events since the last call, and the editor's text now.
  const take = () => ({ events: state.events.splice(0), doc: doc() });

  // RFC-046 section 7's fallback question: can a script build a paste event with
  // text/html and have it reach a handler?
  const construct = () => {
    const out = { ok: false };
    try {
      const transfer = new DataTransfer();
      transfer.setData("text/html", "<h1>Constructed</h1><p>x</p>");
      transfer.setData("text/plain", "Constructed x");
      out.dataTransferTypes = Array.from(transfer.types);
      const event = new ClipboardEvent("paste", {
        clipboardData: transfer,
        bubbles: true,
        cancelable: true,
      });
      out.eventHasClipboardData = !!event.clipboardData;
      out.eventTypes = event.clipboardData ? Array.from(event.clipboardData.types) : null;
      const target = view() ? view().contentDOM : document.activeElement;
      const before = state.events.length;
      const docBefore = doc();
      out.notCancelled = target.dispatchEvent(event);
      out.docChanged = doc() !== docBefore;
      out.reached = state.events.slice(before);
      out.ok = true;
    } catch (error) {
      out.error = String(error);
    }
    return out;
  };

  window.__pasteProbe = { install, take, construct, doc };
  return true;
})();
