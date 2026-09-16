// A simulated overflow menu, standing in for the app menu and the
// editor-tools menu (components/app_bar.rs, components/editor_header.rs) for
// testing shell_behaviour_driver.js's slice-2 phases without a real WebView.
//
// Bounded like the other fakes: it models exactly the behaviour RFC-042 §7.2
// specifies and shell_focus.rs implements -- trigger_key_intent (Down/Enter/
// Space open to the first item, Up to the last), menu_item_key_intent
// (Down/Up wrap, Home/End), Escape closing with focus restored to the
// trigger, and the app frame's focusin closing WITHOUT restoring. Nothing
// else: items have no actions, because the driver must never activate one.

/** One menu item: `role="menuitem"`, `tabindex="-1"`, focus() and keydown. */
class FakeMenuItem {
  constructor(menu, label) {
    this.menu = menu;
    this.label = label;
    this.textContent = label;
    this.nodeType = 1;
    this.dispatchedEvents = [];
  }

  getAttribute(name) {
    if (name === "role") return "menuitem";
    if (name === "tabindex") return "-1";
    return null;
  }

  focus() {
    this.menu.owner.setFocus(this);
  }

  dispatchEvent(event) {
    this.dispatchedEvents.push(event);
    this.menu.handleItemKey(this, event.key);
    return true;
  }
}

/** The trigger button: `aria-expanded`, `aria-haspopup`, focus() and keydown. */
class FakeMenuTrigger {
  constructor(menu) {
    this.menu = menu;
    this.nodeType = 1;
    this.dispatchedEvents = [];
  }

  get id() {
    return this.menu.triggerId;
  }

  getAttribute(name) {
    if (name === "aria-expanded") return String(this.menu.open);
    if (name === "aria-haspopup") return "menu";
    return null;
  }

  focus() {
    this.menu.owner.setFocus(this);
  }

  dispatchEvent(event) {
    this.dispatchedEvents.push(event);
    this.menu.handleTriggerKey(event.key);
    return true;
  }
}

/** The `role="menu"` container, which the driver queries for its items. */
class FakeMenuContainer {
  constructor(menu) {
    this.menu = menu;
    this.nodeType = 1;
  }

  getAttribute(name) {
    return name === "role" ? "menu" : null;
  }

  querySelectorAll(selector) {
    return selector === '[role="menuitem"]' ? this.menu.items : [];
  }

  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }
}

/**
 * `owner` is the FakeTree: it owns `document.activeElement` and the selector
 * map, so a menu and the tree share one focus location, exactly as the real
 * document does.
 *
 * `restoreOnEscape: false` models the mutation where Escape releases without
 * restoring, and `restoreOnFocusLeave: true` models the app frame restoring
 * on focusin -- the C3 defect's shape (slice-2 handoff §10.4).
 */
export class FakeMenu {
  constructor(
    owner,
    {
      triggerSelector,
      menuSelector,
      labels = ["first", "middle", "last"],
      wraps = true,
      restoreOnEscape = true,
      restoreOnFocusLeave = false,
    },
  ) {
    this.owner = owner;
    this.triggerSelector = triggerSelector;
    this.triggerId = triggerSelector.replace(/^#/, "");
    this.menuSelector = menuSelector;
    this.wraps = wraps;
    this.restoreOnEscape = restoreOnEscape;
    this.restoreOnFocusLeave = restoreOnFocusLeave;
    this.open = false;
    this.trigger = new FakeMenuTrigger(this);
    this.container = new FakeMenuContainer(this);
    this.items = labels.map((label) => new FakeMenuItem(this, label));
    this.activations = 0;
  }

  /** Registers the trigger, and the container once opened, with the owner. */
  install() {
    this.owner.setElement(this.triggerSelector, this.trigger);
    this.owner.setElement(this.menuSelector, null);
    this.owner.watchFocus((element) => this.onFocusMoved(element));
    return this;
  }

  contains(element) {
    return element === this.trigger || element === this.container || this.items.includes(element);
  }

  setOpen(open) {
    this.open = open;
    this.owner.setElement(this.menuSelector, open ? this.container : null);
  }

  /** shell_focus::trigger_key_intent, then focus_menu_item. */
  handleTriggerKey(key) {
    if (key === "ArrowDown" || key === "Enter" || key === " ") {
      this.setOpen(true);
      this.owner.setFocus(this.items[0]);
    } else if (key === "ArrowUp") {
      this.setOpen(true);
      this.owner.setFocus(this.items[this.items.length - 1]);
    } else if (key === "Escape" && this.open) {
      this.close({ restore: this.restoreOnEscape });
    }
  }

  /** The menu wrap's onkeydown: Escape, then menu_item_key_intent. */
  handleItemKey(item, key) {
    if (key === "Escape") {
      this.close({ restore: this.restoreOnEscape });
      return;
    }
    if (key === "Enter" || key === " ") {
      // Native button activation. The driver must never reach this (§5).
      this.activations += 1;
      return;
    }
    const index = this.items.indexOf(item);
    const last = this.items.length - 1;
    let target = null;
    if (key === "ArrowDown") {
      target = index === last ? (this.wraps ? 0 : last) : index + 1;
    } else if (key === "ArrowUp") {
      target = index === 0 ? (this.wraps ? last : 0) : index - 1;
    } else if (key === "Home") {
      target = 0;
    } else if (key === "End") {
      target = last;
    }
    if (target !== null) this.owner.setFocus(this.items[target]);
  }

  close({ restore }) {
    this.setOpen(false);
    if (restore) this.owner.setFocus(this.trigger);
  }

  /** The app frame's onfocusin: focus outside the wrap closes the menu, and
   * `release_menu_focus` does not restore. In-menu focus moves stop
   * propagation, so they never reach it. */
  onFocusMoved(element) {
    if (!this.open || this.contains(element)) return;
    this.setOpen(false);
    if (this.restoreOnFocusLeave) this.owner.setFocus(this.trigger);
  }
}
