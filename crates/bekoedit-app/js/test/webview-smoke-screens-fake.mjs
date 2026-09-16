// Simulated Recovery screen, Settings screen and conflict banner, standing in
// for components/recovery_screen.rs, settings_screen.rs and
// conflict_banner.rs, for testing shell_behaviour_driver.js's slice-3
// stage-2 phases (RFC-044 §8 E and F).
//
// Bounded like the other fakes. Each models what its component does to
// focus and nothing more, and each has one option per defect the matching
// contract must catch, arriving one animation frame late where the contract
// is an absence (slice 3 handoff §9.2).

const focusable = (owner, id) => {
  const element = { id, nodeType: 1, clicks: 0 };
  element.focus = () => owner.setFocus(element);
  return element;
};

/**
 * One seeded snapshot. `focusHeading: false` models entry focus not landing
 * (E1). `restoreToLogo: false` models `use_drop` not restoring (E2).
 * `stealFocusNextFrame: true` models a focus move that takes focus from the
 * logo one frame after Recovery exits (E2's absence).
 */
export class FakeRecovery {
  constructor(owner, { count = 1, focusHeading = true, restoreToLogo = true, stealFocusNextFrame = false } = {}) {
    this.owner = owner;
    this.count = count;
    this.focusHeading = focusHeading;
    this.restoreToLogo = restoreToLogo;
    this.stealFocusNextFrame = stealFocusNextFrame;
    this.heading = focusable(owner, "recovery-heading");
    this.status = { nodeType: 1, textContent: `${count} recoverable` };
    this.skip = focusable(owner, null);
    this.skip.click = () => this.skipAll();
    this.logo = focusable(owner, "app-bar-logo-trigger");
    this.elsewhere = focusable(owner, "stolen-focus");
    this.region = {
      nodeType: 1,
      querySelector: (selector) => (selector === '[role="status"]' ? this.status : null),
      contains: (element) => [this.heading, this.status, this.skip].includes(element),
    };
    this.skipped = 0;
  }

  install() {
    this.owner.setElement('[role="region"][aria-labelledby="recovery-heading"]', this.region);
    this.owner.setElements(".recovery-skip", [this.skip]);
    this.owner.setElement("#app-bar-logo-trigger", this.logo);
    if (this.focusHeading) this.owner.setFocus(this.heading);
    return this;
  }

  skipAll() {
    this.skip.clicks += 1;
    this.skipped += 1;
    this.owner.setElement('[role="region"][aria-labelledby="recovery-heading"]', null);
    this.owner.setElements(".recovery-skip", []);
    if (this.restoreToLogo) this.owner.setFocus(this.logo);
    if (this.stealFocusNextFrame) {
      requestAnimationFrame(() => this.owner.setFocus(this.elsewhere));
    }
  }
}

/**
 * `menu` is the app menu's FakeMenu; `tabs` is the FakeModeTabs whose editor
 * unmounts while Settings replaces the shell and remounts on Close.
 * `restoreToTrigger: false` models `close_settings` not restoring (E4).
 * `editorTakesFocusNextFrame: true` models the remounted editor taking focus
 * one frame after the restore (E4's absence).
 */
export class FakeSettings {
  constructor(owner, { menu, tabs, restoreToTrigger = true, editorTakesFocusNextFrame = false } = {}) {
    this.owner = owner;
    this.menu = menu;
    this.tabs = tabs;
    this.restoreToTrigger = restoreToTrigger;
    this.editorTakesFocusNextFrame = editorTakesFocusNextFrame;
    this.item = focusable(owner, "app-menu-settings");
    this.item.click = () => this.open();
    this.heading = focusable(owner, "settings-heading");
    this.close = focusable(owner, "settings-close");
    this.close.click = () => this.closeScreen();
    this.save = focusable(owner, null);
    this.region = {
      nodeType: 1,
      contains: (element) => [this.heading, this.close, this.save].includes(element),
    };
    this.opened = 0;
    this.closed = 0;
  }

  install() {
    this.menu.extraContained.push(this.item);
    this.owner.setElements("#app-menu-settings", [this.item]);
    this.owner.setElements("#settings-close", []);
    return this;
  }

  /** The Settings item's onclick: close the menu without restoring, replace
   * the shell, focus the heading. */
  open() {
    this.item.clicks += 1;
    this.opened += 1;
    this.menu.setOpen(false);
    this.owner.setElement('[data-source-focus-launch-region="text"]', null);
    this.owner.setEditorView(undefined);
    this.owner.setElement('[role="region"][aria-labelledby="settings-heading"]', this.region);
    this.owner.setElements("#settings-close", [this.close]);
    this.owner.setFocus(this.heading);
  }

  /** `close_settings`: release, restore to the app-menu trigger, reopen the
   * shell, whose Text-mode editor remounts. */
  closeScreen() {
    this.close.clicks += 1;
    this.closed += 1;
    this.owner.setElement('[role="region"][aria-labelledby="settings-heading"]', null);
    this.owner.setElements("#settings-close", []);
    if (this.restoreToTrigger) this.owner.setFocus(this.menu.trigger);
    this.tabs.renderMode();
    if (this.editorTakesFocusNextFrame) {
      requestAnimationFrame(() => this.owner.setFocus(this.tabs.content));
    }
  }
}

/**
 * The header's file name and dirty dot, and the conflict banner the Rust
 * sequence's on-disk write produces. `tabs.onDispatch` marks the document
 * dirty. `showBanner()` stands in for the write plus the 500 ms poll.
 * `buttons: 2` is the deleted-file banner; `focusFirstActionNextFrame: true`
 * models RFC-042 §7.6's withdrawn text (F2's absence).
 */
export class FakeConflict {
  constructor(owner, { tabs, fileName = "child.md", buttons = 3, focusFirstActionNextFrame = false } = {}) {
    this.owner = owner;
    this.tabs = tabs;
    this.fileName = { nodeType: 1, textContent: fileName };
    this.dirtyDot = { nodeType: 1 };
    this.dispatches = 0;
    this.actions = Array.from({ length: buttons }, () => focusable(owner, null));
    for (const action of this.actions) {
      action.click = () => {
        action.clicks += 1;
      };
    }
    this.focusFirstActionNextFrame = focusFirstActionNextFrame;
    this.banner = {
      nodeType: 1,
      querySelectorAll: (selector) => (selector === "button" ? this.actions : []),
      contains: (element) => this.actions.includes(element),
    };
  }

  install() {
    this.owner.setElement(".file-name", this.fileName);
    this.tabs.onDispatch = () => {
      this.dispatches += 1;
      this.owner.setElement(".dirty-dot", this.dirtyDot);
    };
    return this;
  }

  showBanner() {
    this.owner.setElement('.conflict-banner[role="alert"]', this.banner);
    if (this.focusFirstActionNextFrame) {
      requestAnimationFrame(() => this.owner.setFocus(this.actions[0]));
    }
  }
}
