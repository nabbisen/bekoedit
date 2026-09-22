// A simulated mode tablist and source editor, standing in for
// components/editor_header/mode_tabs.rs and the Text-mode CodeMirror view,
// for testing shell_behaviour_driver.js's slice-3 stage-1 phases.
//
// Bounded like the other fakes: Right/Left wrap and Home/End move focus only
// (tab_key_intent + focus_tab); a click runs the tab's onclick, which selects
// its mode; switching to Text mounts the editor and claims its focus. The
// claim follows the source focus guard's rule for a persistent control: it
// is accepted only while the clicked tab holds focus, and refused while
// shell authority is held. Nothing else.

const TAB_ORDER = ["mode-text", "mode-preview", "mode-form"];

class FakeTab {
  constructor(tabs, launchId) {
    this.tabs = tabs;
    this.launchId = launchId;
    this.nodeType = 1;
    this.clicks = 0;
    this.dispatchedEvents = [];
  }

  getAttribute(name) {
    if (name === "data-source-focus-launch") return this.launchId;
    if (name === "role") return "tab";
    if (name === "aria-selected") return String(this.tabs.selected === this.launchId);
    if (name === "tabindex") return this.tabs.selected === this.launchId ? "0" : "-1";
    return null;
  }

  focus() {
    this.tabs.owner.setFocus(this);
  }

  /** The tablist's onkeydown, reached by bubbling. */
  dispatchEvent(event) {
    this.dispatchedEvents.push(event);
    this.tabs.handleKey(this, event.key);
    return true;
  }

  /** The tab's onclick: SwitchMode for its mode. */
  click() {
    this.clicks += 1;
    this.tabs.activate(this);
  }
}

class FakeTabList {
  constructor(tabs) {
    this.tabs = tabs;
    this.nodeType = 1;
  }

  querySelectorAll(selector) {
    return selector === '[role="tab"]' ? this.tabs.elements : [];
  }
}

/**
 * `owner` is the FakeTree, which owns `document.activeElement`.
 *
 * Options, each modelling a defect a stage-1 contract must catch:
 * - `activateOnArrow: "nextFrame"` -- automatic activation, one frame after
 *   the focus move (C1's absence).
 * - `authorityHeld` (settable) -- shell authority still held, so the Text
 *   tab's claim is refused and the editor never takes focus (D2).
 * - `teardownLag: true` -- task 021's race. Leaving Text renders the new mode
 *   at once, but the controller stays in `Unmounting` until `finishTeardown()`
 *   (the page's `destroyed` event reaching Rust). What a Text activation
 *   inside that window does next is `queueDuringTeardown`'s call:
 *   - `false` (default) -- dropped as `Busy`, the pre-RFC-047 behaviour, so
 *     the mode stays where it was. Task 021's settle gate is what waits for
 *     `finishTeardown()`'s real counterpart instead of acting into this
 *     window; a test stands in for it by calling `finishTeardown()` between
 *     exchanges.
 *   - `true` -- queued instead (RFC-047 slice 1), and replayed by
 *     `finishTeardown()` once the window ends, with no exchange boundary
 *     required in between. RFC-047 §7's phase exercises this: it clicks
 *     Preview and Text with nothing waiting between them, in one exchange,
 *     so only the queue -- not the gate -- can make Text win.
 */
export class FakeModeTabs {
  constructor(
    owner,
    {
      selected = "mode-form",
      activateOnArrow = null,
      docLength = 8,
      teardownLag = false,
      queueDuringTeardown = false,
    } = {},
  ) {
    this.owner = owner;
    this.selected = selected;
    this.activateOnArrow = activateOnArrow;
    this.docLength = docLength;
    this.teardownLag = teardownLag;
    this.queueDuringTeardown = queueDuringTeardown;
    this.tearingDown = false;
    /** Activations dropped as Busy inside the teardown window
     * (`queueDuringTeardown: false`, the pre-RFC-047 shape). */
    this.droppedActivations = 0;
    /** The one activation queued inside the teardown window
     * (`queueDuringTeardown: true`), replayed by `finishTeardown()`. */
    this.queuedActivation = null;
    this.authorityHeld = false;
    /** Called on every view.dispatch, so a conflict fake can mark the
     * document dirty (slice 3 §8 F1). */
    this.onDispatch = null;
    this.elements = TAB_ORDER.map((launchId) => new FakeTab(this, launchId));
    this.list = new FakeTabList(this);
    this.content = { nodeType: 1, focus: () => this.owner.setFocus(this.content) };
    this.host = { nodeType: 1, querySelector: () => null };
    this.view = null;
  }

  install() {
    this.owner.setElement("#editor-mode-switch", this.list);
    for (const tab of this.elements) {
      this.owner.setElements(`[data-source-focus-launch="${tab.launchId}"]`, [tab]);
    }
    this.renderMode();
    return this;
  }

  tab(launchId) {
    return this.elements.find((tab) => tab.launchId === launchId);
  }

  /** Mounts the Text-mode editor host and view, or unmounts them. */
  renderMode() {
    if (this.selected === "mode-text") {
      const content = this.content;
      const owner = this.owner;
      this.view = {
        dom: { isConnected: true },
        contentDOM: content,
        state: { doc: { length: this.docLength } },
        get hasFocus() {
          return owner.activeElement() === content;
        },
        focus: () => owner.setFocus(content),
        dispatch: (patch) => this.onDispatch?.(patch),
      };
      this.owner.setElement('[data-source-focus-launch-region="text"]', this.host);
      this.owner.setEditorView(this.view);
    } else {
      this.view = null;
      this.owner.setElement('[data-source-focus-launch-region="text"]', null);
      this.owner.setEditorView(undefined);
    }
  }

  handleKey(tab, key) {
    const index = this.elements.indexOf(tab);
    const last = this.elements.length - 1;
    let target = null;
    if (key === "ArrowRight") target = index === last ? 0 : index + 1;
    else if (key === "ArrowLeft") target = index === 0 ? last : index - 1;
    else if (key === "Home") target = 0;
    else if (key === "End") target = last;
    if (target === null) return;
    const next = this.elements[target];
    this.owner.setFocus(next);
    if (this.activateOnArrow === "nextFrame") {
      requestAnimationFrame(() => this.select(next.launchId));
    }
  }

  select(launchId) {
    const leavingText = this.selected === "mode-text" && launchId !== "mode-text";
    this.selected = launchId;
    this.renderMode();
    if (this.teardownLag && leavingText) this.tearingDown = true;
  }

  /** The `destroyed` event reaching Rust: the controller leaves Unmounting,
   * and the queue (if anything is in it) drains. */
  finishTeardown() {
    this.tearingDown = false;
    if (this.queuedActivation) {
      const tab = this.queuedActivation;
      this.queuedActivation = null;
      this.activate(tab);
    }
  }

  activate(tab) {
    if (this.tearingDown && tab.launchId === "mode-text") {
      if (this.queueDuringTeardown) {
        this.queuedActivation = tab;
      } else {
        this.droppedActivations += 1;
      }
      return;
    }
    const focusedOnTab = this.owner.activeElement() === tab;
    this.select(tab.launchId);
    if (tab.launchId === "mode-text" && focusedOnTab && !this.authorityHeld) {
      this.owner.setFocus(this.content);
    }
  }
}
