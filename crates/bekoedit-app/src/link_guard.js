// Link click guard (task 032). Run once by `link_guard::use_link_guard`.
//
// Dioxus's page interpreter turns a click inside any <a> into
// `webbrowser::open(href)` with the raw `href`, so a relative link
// reached the OS opener as a path. This listener runs first (capture phase,
// on `window`), cancels the click before the interpreter's own listeners are
// reached, and hands the `href` to Rust. Rust decides what the click does
// (`bekoedit_core::decide_link_click`); nothing here opens anything.
//
// A middle click or a modified click is cancelled too, and sends nothing.
(async () => {
  if (window.__bk_link_guard) {
    window.removeEventListener("click", window.__bk_link_guard, true);
    window.removeEventListener("auxclick", window.__bk_link_guard, true);
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
      dioxus.send(anchor.getAttribute("href") ?? "");
    }
  };
  window.__bk_link_guard = guard;
  window.addEventListener("click", guard, true);
  window.addEventListener("auxclick", guard, true);
  // Keep the evaluator alive, so the channel back to Rust stays open.
  await new Promise(() => {});
})();
