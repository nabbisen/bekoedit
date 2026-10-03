/**
 * Global keyboard shortcuts for bekoedit (RFC-020).
 * Installed once by the App shell; works regardless of which component
 * has DOM focus.
 *
 * Shortcuts forwarded to Rust as:
 *   dioxus.send(JSON.stringify({type:"shortcut", key, composing}))
 * where `key` matches the action names in the Rust handler.
 *
 * Text-Mode editing keys (Ctrl+Z, Ctrl+F, etc.) are handled by CM6 directly
 * when the editor has focus; this script handles app-level actions only.
 */
(function () {
  const isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform);
  const mod = (e) => isMac ? e.metaKey : e.ctrlKey;

  window.addEventListener("keydown", (e) => {
    if (!mod(e)) return;
    let key = null;

    if (e.key === "s" || e.key === "S") { key = "save"; }
    else if (e.key === "1") { key = "mode_text"; }
    else if (e.key === "2") { key = "mode_form"; }
    else if (e.key === "3") { key = "mode_preview"; }
    else if (e.key === "b" || e.key === "B") { key = "toggle_explorer"; }
    else if (e.key === "4") { key = "mode_split"; }

    if (key) {
      e.preventDefault();
      // Task 051 §2.1: a Form field commits only on `onchange`, which
      // fires on blur -- a keyboard shortcut never touches DOM focus, so
      // without this the field's pending text would be left behind.
      // Dioxus delivers this `change` to Rust through a *synchronous*
      // XHR (`dioxus-interpreter-js`'s `handleVirtualdomEventSync`), so
      // by the time `dispatchEvent` returns here, Rust has already
      // committed the text -- well before the relay below even runs.
      //
      // Task 054 §2.3: `toggle_explorer` is not a source command -- it
      // saves or leaves nothing, so it must never flush a Form field or
      // be refused for composing.
      let composing = false;
      if (key !== "toggle_explorer") {
        const el = document.activeElement;
        if (el && typeof el.value === "string" && el.closest && el.closest(".form-mode")) {
          composing = e.isComposing || window.__bk_form_composing === true;
          if (!composing) {
            el.dispatchEvent(new Event("change", { bubbles: true }));
          }
        }
      }
      // Task 054 §2.1: this used to be gated on a global this framework
      // has never defined -- not in this version, not in bekoedit's own
      // code. The relay itself binds to the eval's own *local* channel
      // (`bridge::relay_js`), never that global. No keyboard shortcut
      // reached Rust since that gate was added (`909f84d`, before
      // 0.10.0); the optional call below already does nothing when the
      // relay has not (yet) installed itself.
      window.__bk_shortcut_relay?.(JSON.stringify({ type: "shortcut", key, composing }));
    }
  });

  // A Form Mode field's pending text must never be force-committed
  // mid-IME-composition (task 051 §2.1 reads this to decide whether to
  // flush above). Captured at the window level, like the shortcut
  // listener above, so it sees every field regardless of which one has
  // focus.
  window.__bk_form_composing = false;
  window.addEventListener("compositionstart", () => { window.__bk_form_composing = true; }, true);
  window.addEventListener("compositionend", () => { window.__bk_form_composing = false; }, true);
})();
