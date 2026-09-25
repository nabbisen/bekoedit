// Link click guard (task 032). Run once by `link_guard::use_link_guard`.
//
// Dioxus's page interpreter turns a click inside any <a> into
// `webbrowser::open(href)` with the raw `href`, so a relative link
// reached the OS opener as a path. Two independent layers stop that:
//
//  1. This listener runs first (capture phase, on `window`), cancels the
//     click before the interpreter's own listeners are reached, and hands the
//     `href` to Rust. Rust decides what the click does
//     (`bekoedit_core::decide_link_click`); nothing here opens anything.
//  2. The interpreter's own link route is switched off. A click the listener
//     somehow missed then falls to an ordinary navigation, which wry's
//     navigation handler (dioxus-desktop `webview.rs`) opens in the browser
//     only for `http(s):` and `mailto:`, and refuses otherwise.
//
// A middle click or a modified click is cancelled too, and sends nothing.
//
// Messages to Rust are `{ kind: "click", href }` and `{ kind: "trace", detail }`.
(async () => {
  if (window.__bk_link_guard) {
    window.removeEventListener("click", window.__bk_link_guard, true);
    window.removeEventListener("auxclick", window.__bk_link_guard, true);
  }
  // Audited against dioxus-interpreter-js 0.7.9: `NativeInterpreter` sets the
  // plain field `intercept_link_redirects = true` in `initialize`
  // (src/ts/native.ts:43) and reads it in `handleClickNavigate`
  // (native.ts:411-427). The instance is `window.interpreter`. Re-audit both
  // on any Dioxus upgrade. If either is gone, say so instead of going quiet:
  // layer 1 still holds, but it is then the only layer.
  const interpreter = window.interpreter;
  if (interpreter && typeof interpreter.intercept_link_redirects === "boolean") {
    interpreter.intercept_link_redirects = false;
  } else {
    dioxus.send({
      kind: "trace",
      detail: "the interpreter's link route could not be switched off",
    });
  }
  const guard = (event) => {
    const start = event.target;
    const anchor =
      start && typeof start.closest === "function" ? start.closest("a") : null;
    if (!anchor) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.type === "click") {
      dioxus.send({ kind: "click", href: anchor.getAttribute("href") ?? "" });
    }
  };
  window.__bk_link_guard = guard;
  window.addEventListener("click", guard, true);
  window.addEventListener("auxclick", guard, true);
  // Keep the evaluator alive, so the channel back to Rust stays open.
  await new Promise(() => {});
})();
