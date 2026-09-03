// A small simulated workspace tree, standing in for the real app's
// roving-tabindex `[data-tree-row]` rows and their onkeydown handling
// (components/explorer/tree_row.rs, tree_nav.rs), for testing
// shell_behaviour_driver.js without a real WebView or Dioxus tree.
//
// Bounded like webview-smoke-dom-fake.mjs's FakeDom (task 012 §3.4): this
// models exactly the navigation rules tree_nav::navigate implements --
// Up/Down/Home/End move; Right expands then enters; Left collapses then
// ascends; Enter toggles a directory or opens an openable file -- against a
// fixed two-level fixture, not a general filesystem.

/** One visible row's DOM-observable surface: `tabindex`, `aria-expanded`,
 * `aria-disabled`, `focus()`, and `dispatchEvent` wired back into the
 * owning `FakeTree`. Identity is stable across renders (see
 * `FakeTree.elements()`) -- like a real DOM node kept alive by Dioxus's own
 * `key: "{node.path.display()}"`, this object survives sibling rows being
 * added or removed, and only `.index` is refreshed to match its current
 * position. Without that, `rows()[0] === document.activeElement`-style
 * checks the driver makes would never hold, since two separate
 * `document.querySelectorAll` calls would otherwise return different
 * object identities for the same logical row. */
class FakeTreeRowElement {
  constructor(tree, key) {
    this.tree = tree;
    this.key = key;
    this.index = -1;
    this.nodeType = 1; // Node.ELEMENT_NODE
    this.dispatchedEvents = [];
  }

  getAttribute(name) {
    const { node, depth } = this.tree.visibleRows()[this.index];
    if (name === "tabindex") return this.index === this.tree.activeIndex ? "0" : "-1";
    if (name === "aria-expanded") return node.isDir ? String(node.isExpanded) : "false";
    if (name === "aria-disabled") return String(!(node.isDir || node.isOpenable));
    if (name === "data-depth") return String(depth);
    return null;
  }

  focus() {
    this.tree.activeIndex = this.index;
  }

  dispatchEvent(event) {
    this.dispatchedEvents.push(event);
    this.tree.handleKey(this.index, event.key);
    return true;
  }
}

/** A row's stable identity across renders -- mirrors the real app's own
 * `key: "{node.path.display()}"` (components/explorer.rs). */
function rowKey(node, depth) {
  return `${depth}:${node.name}`;
}

/** `workspace/` fixture matching the real second run's seeded profile:
 * `sub/child.md`, `a.md`, `notes.txt` (non-markdown, non-openable), `z.md`
 * -- sorted directories-first, name-ascending, as `dioxus-swdir-tree-core`
 * actually sorts (scan.rs). */
function defaultFixture() {
  return [
    {
      name: "sub",
      isDir: true,
      isExpanded: false,
      children: [{ name: "child.md", isDir: false, isOpenable: true }],
    },
    { name: "a.md", isDir: false, isOpenable: true },
    { name: "notes.txt", isDir: false, isOpenable: false },
    { name: "z.md", isDir: false, isOpenable: true },
  ];
}

export class FakeTree {
  constructor(nodes = defaultFixture()) {
    this.nodes = nodes;
    this.activeIndex = 0;
    this.openedPath = null;
  }

  /** Flattened `(node, depth)` pairs in render order, expanding only
   * directories with `isExpanded: true` -- the same shape
   * `visible_rows()` produces. */
  visibleRows() {
    const out = [];
    const walk = (nodes, depth) => {
      for (const node of nodes) {
        out.push({ node, depth });
        if (node.isDir && node.isExpanded) walk(node.children, depth + 1);
      }
    };
    walk(this.nodes, 0);
    return out;
  }

  /** `[data-tree-row]` elements for the current render: same object
   * identity as the previous call for any row that is still visible
   * (keyed, like the real tree's own `key:` prop), a new object only for
   * a row that just became visible. `.index` is refreshed to the row's
   * current position either way. */
  elements() {
    this._elementsByKey ??= new Map();
    const rows = this.visibleRows();
    const seen = new Set();
    const elements = rows.map(({ node, depth }, index) => {
      const key = rowKey(node, depth);
      seen.add(key);
      let element = this._elementsByKey.get(key);
      if (!element) {
        element = new FakeTreeRowElement(this, key);
        this._elementsByKey.set(key, element);
      }
      element.index = index;
      return element;
    });
    for (const key of [...this._elementsByKey.keys()]) {
      if (!seen.has(key)) this._elementsByKey.delete(key);
    }
    return elements;
  }

  activeElement() {
    return this.elements()[this.activeIndex] ?? null;
  }

  /** Mirrors tree_row.rs's onkeydown match arms + tree_nav::navigate. */
  handleKey(index, key) {
    const visible = this.visibleRows();
    const row = visible[index];
    switch (key) {
      case "ArrowDown":
        if (index + 1 < visible.length) this.activeIndex = index + 1;
        break;
      case "ArrowUp":
        if (index > 0) this.activeIndex = index - 1;
        break;
      case "Home":
        if (index !== 0) this.activeIndex = 0;
        break;
      case "End":
        if (index !== visible.length - 1) this.activeIndex = visible.length - 1;
        break;
      case "ArrowRight":
        if (!row.node.isDir) break;
        if (!row.node.isExpanded) {
          row.node.isExpanded = true;
          this.activeIndex = index;
        } else {
          const childDepth = row.depth + 1;
          const next = this.visibleRows()[index + 1];
          if (next && next.depth === childDepth) this.activeIndex = index + 1;
        }
        break;
      case "ArrowLeft":
        if (row.node.isDir && row.node.isExpanded) {
          row.node.isExpanded = false;
          this.activeIndex = index;
        } else {
          for (let i = index - 1; i >= 0; i -= 1) {
            if (visible[i].depth < row.depth) {
              this.activeIndex = i;
              break;
            }
          }
        }
        break;
      case "Enter":
      case " ":
        if (row.node.isDir) {
          row.node.isExpanded = !row.node.isExpanded;
          this.activeIndex = index;
        } else if (row.node.isOpenable) {
          this.openedPath = row.node.name;
        }
        break;
      default:
        break;
    }
  }

  /** Installs `document`/`window`/`KeyboardEvent`/`Node`/`performance`/
   * `requestAnimationFrame` shaped exactly like what
   * shell_behaviour_driver.js reads or calls, backed by this tree. */
  install() {
    const tree = this;
    globalThis.window = { __bk: undefined };
    globalThis.Node = { ELEMENT_NODE: 1 };
    globalThis.KeyboardEvent = class KeyboardEvent {
      constructor(type, init) {
        this.type = type;
        Object.assign(this, init);
      }
    };
    globalThis.MutationObserver = class MutationObserver {
      observe() {}
      disconnect() {}
    };
    let time = 0;
    globalThis.performance = { now: () => time };
    this.setTime = (ms) => {
      time = ms;
    };
    // A real requestAnimationFrame corresponds to elapsed wall-clock time
    // (~16ms/frame); advancing the fake clock here is what lets the
    // driver's own waitFor() poll loop actually reach its deadline and
    // time out, instead of spinning forever against a clock nothing ever
    // moves (setTime is for the *state.deadline* checks the driver makes
    // explicitly, a separate, coarser clock read).
    globalThis.requestAnimationFrame = (callback) =>
      setImmediate(() => {
        time += 16;
        callback(performance.now());
      });
    globalThis.document = {
      documentElement: {},
      querySelector: (selector) => {
        if (selector === ".toast-error") return this.errorToastElement ?? null;
        return this.elementsBySelector?.[selector] ?? null;
      },
      querySelectorAll: (selector) => (selector === "[data-tree-row]" ? tree.elements() : []),
      get activeElement() {
        return tree.activeElement();
      },
    };
  }

  /** `window.__bk._view` will be `view` (or `undefined`), matching
   * `FakeEditorView` from webview-smoke-dom-fake.mjs. */
  setEditorView(view) {
    window.__bk = view === undefined ? undefined : { _view: view };
  }

  /** `document.querySelector(selector)` returns `element` for selectors
   * other than `.toast-error` (routed to `errorToastElement`/none). */
  setElement(selector, element) {
    this.elementsBySelector ??= {};
    this.elementsBySelector[selector] = element ?? null;
  }
}
