(async () => {
  const marker = "RFC041_WEBVIEW_SMOKE_MARKER";
  const milestones = [];
  const deadline = performance.now() + 15000;
  let stage = "observer_install";
  let errorToastSeen = Boolean(document.querySelector(".toast-error"));
  const containsErrorToast = (node) =>
    node?.nodeType === Node.ELEMENT_NODE &&
    (node.matches?.(".toast-error") || node.querySelector?.(".toast-error"));
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      if (
        [...record.addedNodes].some(containsErrorToast) ||
        containsErrorToast(record.target)
      ) {
        errorToastSeen = true;
      }
    }
  });
  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class"],
  });
  milestones.push("observer_installed");

  const waitFor = async (name, probe) => {
    stage = name;
    while (performance.now() < deadline) {
      const value = probe();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    throw new Error(`timed out at ${name}`);
  };
  const click = (element) =>
    element.dispatchEvent(
      new MouseEvent("click", {
        view: window,
        bubbles: true,
        cancelable: true,
        button: 0,
      }),
    );

  try {
    const newButton = await waitFor("start_visible", () =>
      document.querySelector('[data-source-focus-launch="start-new"]'),
    );
    milestones.push("start_visible");
    click(newButton);
    milestones.push("new_clicked");

    const view = await waitFor("editor_ready_focused", () => {
      const current = window.__bk?._view;
      const host = document.querySelector(
        '[data-source-focus-launch-region="text"]',
      );
      return current &&
        current.dom?.isConnected &&
        current.hasFocus &&
        host &&
        !host.querySelector(".source-editor-status")
        ? current
        : null;
    });
    milestones.push("editor_ready_focused");

    stage = "edit_dispatched";
    view.dispatch({ changes: { from: view.state.doc.length, insert: marker } });
    milestones.push("edit_dispatched");
    const previewButton = document.querySelector(
      '[data-source-focus-launch="mode-preview"]',
    );
    if (!previewButton) throw new Error("Preview control is missing");
    click(previewButton);
    milestones.push("preview_clicked");

    const preview = await waitFor("preview_verified", () => {
      const article = document.querySelector("article.preview");
      const active = document.querySelector(
        '[data-source-focus-launch="mode-preview"].active[aria-selected="true"]',
      );
      return article?.textContent?.includes(marker) && active ? article : null;
    });
    if (!preview.textContent.includes(marker)) {
      throw new Error("Preview does not contain the exact marker");
    }
    if (errorToastSeen) throw new Error("an error toast appeared");
    milestones.push("preview_verified");
    stage = "preview_verified";
    dioxus.send({
      ok: true,
      stage,
      marker,
      milestones,
      errorToastSeen,
      error: null,
    });
  } catch (error) {
    dioxus.send({
      ok: false,
      stage,
      marker,
      milestones,
      errorToastSeen,
      error: String(error),
    });
  } finally {
    observer.disconnect();
  }
})();
