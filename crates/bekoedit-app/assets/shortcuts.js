/**
 * Global keyboard shortcuts for bekoedit (RFC-020).
 * Installed once by the App shell; works regardless of which component
 * has DOM focus.
 *
 * Shortcuts forwarded to Rust as: dioxus.send(JSON.stringify({type:"shortcut", key}))
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
      if (window.dioxus) {
        window.__bk_shortcut_relay?.(JSON.stringify({ type: "shortcut", key }));
      }
    }
  });

  // Task 048 D1: a Form Mode field's pending text must never be
  // force-committed mid-IME-composition. Captured at the window level, like
  // the shortcut listener above, so it sees every field regardless of which
  // one has focus.
  window.__bk_form_composing = false;
  window.addEventListener("compositionstart", () => { window.__bk_form_composing = true; }, true);
  window.addEventListener("compositionend", () => { window.__bk_form_composing = false; }, true);
})();
