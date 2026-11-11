"""The pure part of the accessibility-tree observer (RFC-050 slice 1, level 3).

`scripts/a11y-observer.py` reads the AT-SPI tree of the running app and acts on it;
this module is everything that does not need a display or `gi`: how a tree is
represented, how a control is found by role and exact name, how the launched
application is picked by process id, what each step of the check requires, and
what an abort report says. It is unit-tested on recorded trees
(`scripts/test-a11y-observer.py`), in `lint`, with no display.

A tree is plain data, one dict per accessible object:

    {"role": "push button",        # AT-SPI's role name, as WebKit gives it
     "name": "保存",
     "states": ["enabled", "showing"],   # AT-SPI state nicks
     "actions": ["click"],               # AT-SPI action names
     "attrs": {"container-live": "polite"},
     "text": "…",                        # the text interface's text, if any
     "pid": 1234,                        # on application nodes
     "children": [...]}

Expected names are never typed here: they arrive in a JSON file written by the app
itself from `i18n.rs`'s own lookup (`names`, keyed by the same keys the app uses).
"""

import json

# ARIA role -> the AT-SPI role names it may be exposed as. The check matches on
# what WebKit really gives, and reports it; this table is where a real run's
# finding is recorded once seen. An unknown role is a finding, never a guess.
ROLE_ALIASES = {
    "application": ["application"],
    "window": ["frame", "window", "dialog"],
    "button": ["push button", "button"],
    "tab": ["page tab"],
    "tablist": ["page tab list"],
    "tree": ["tree"],
    "treeitem": ["tree item"],
    "complementary": ["landmark", "complementary"],
    "toolbar": ["tool bar"],
    "textbox": ["entry", "text", "paragraph"],
    "combobox": ["combo box"],
    "checkbox": ["check box"],
    "spinbutton": ["spin button"],
    "menu": ["menu"],
    "menuitem": ["menu item"],
    "region": ["landmark", "section", "panel", "grouping"],
    "heading": ["heading"],
    "group": ["panel", "grouping", "section"],
    "table": ["table"],
    "columnheader": ["column header", "table column header"],
    "status": ["status bar", "notification", "static", "label", "section", "panel"],
}

# Required keys of the expectations file's `names` (checked against the app's own
# list by the unit tests, so the two cannot drift).
REQUIRED_KEYS = [
    "app.title",
    "explorer.label",
    "explorer.tree_label",
    "explorer.new_file",
    "mode.text",
    "mode.form",
    "mode.preview",
    "editor.mode_label",
    "editor.save",
    "menu.app",
    "settings.title",
    "settings.language",
    "settings.close",
    "block.kind.heading",
    "block.kind.paragraph",
    "block.kind.table",
    "form.undo",
    "form.redo",
    "save.clean",
    "save.dirty",
    "save.scheduled",
    "save.saving",
    "save.saved",
    "save.failed",
    "save.conflict",
    "toast.dismiss",
]


class Anomaly(Exception):
    """The first thing that is not as required. Carries what the report needs."""

    def __init__(self, step, condition, found):
        super().__init__(f"{step}: {condition}")
        self.step = step
        self.condition = condition
        self.found = found


def walk(node):
    """`node` and every descendant, in document order."""
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def role_matches(aria_role, atspi_role):
    return atspi_role in ROLE_ALIASES.get(aria_role, [aria_role])


def find_all(tree, aria_role, name=None):
    return [
        n
        for n in walk(tree)
        if role_matches(aria_role, n.get("role", "")) and (name is None or n.get("name") == name)
    ]


def nearest(tree, aria_role, limit=12):
    """What is on the page with this role, or with a similar name, for a report."""
    same = [f"{n.get('role')}:{n.get('name')!r}" for n in find_all(tree, aria_role)]
    return same[:limit]


def pick_application(applications, pid):
    """The one application whose process id is the launched pid.

    Never "the first application named bekoedit": a stale or second instance with
    the same name must not be mistaken for the one under test."""
    matching = [a for a in applications if a.get("pid") == pid]
    if len(matching) != 1:
        raise Anomaly(
            "pick the application",
            f"exactly one application has process id {pid}",
            {
                "found": len(matching),
                "applications": [f"{a.get('name')!r} (pid {a.get('pid')})" for a in applications],
            },
        )
    return matching[0]


def _need(tree, step, aria_role, name, problems, label=None):
    found = find_all(tree, aria_role, name)
    if not found:
        problems.append(
            {
                "step": step,
                "condition": f"a {aria_role} named {name!r}" + (f" ({label})" if label else ""),
                "found": nearest(tree, aria_role),
            }
        )
        return None
    return found[0]


def _selected(node):
    return "selected" in node.get("states", [])


def shell_problems(tree, names, files):
    """What the shell must expose once the workspace is open in Text Mode."""
    problems = []
    step = "the shell is in the tree"
    windows = [n for n in walk(tree) if role_matches("window", n.get("role", "")) and names["app.title"].lower() in (n.get("name") or "").lower()]
    if not windows:
        problems.append({"step": step, "condition": f"a window whose title names {names['app.title']!r}", "found": nearest(tree, "window")})
    _need(tree, step, "complementary", names["explorer.label"], problems, "explorer.label")
    _need(tree, step, "tree", names["explorer.tree_label"], problems, "explorer.tree_label")
    _need(tree, step, "button", names["explorer.new_file"], problems, "explorer.new_file")
    for file_name in files:
        row = _need(tree, step, "treeitem", file_name, problems, "a seeded file's row")
        if row is not None and not row.get("actions"):
            problems.append({"step": step, "condition": f"the row {file_name!r} has an action to open it", "found": row})
    _need(tree, step, "tablist", names["editor.mode_label"], problems, "editor.mode_label")
    tabs = {key: _need(tree, step, "tab", names[key], problems, key) for key in ("mode.text", "mode.form", "mode.preview")}
    if all(tabs.values()):
        selected = [names[key] for key, tab in tabs.items() if _selected(tab)]
        if selected != [names["mode.text"]]:
            problems.append({"step": step, "condition": f"exactly one tab selected, {names['mode.text']!r}", "found": selected})
    save = _need(tree, step, "button", names["editor.save"], problems, "editor.save")
    if save is not None and not save.get("actions"):
        problems.append({"step": step, "condition": "the Save control has an action", "found": save})
    _need(tree, step, "button", names["menu.app"], problems, "menu.app")
    return problems


def editor_node(tree, document_text):
    """The editable text object holding the document, or `None`."""
    probe = document_text.strip().splitlines()[0] if document_text.strip() else ""
    for node in walk(tree):
        if "editable" in node.get("states", []) and role_matches("textbox", node.get("role", "")) and probe in (node.get("text") or ""):
            return node
    return None


def editor_problems(tree, document_text):
    node = editor_node(tree, document_text)
    if node is None:
        editable = [f"{n.get('role')}:{n.get('name')!r}:{(n.get('text') or '')[:30]!r}" for n in walk(tree) if "editable" in n.get("states", [])]
        return [{"step": "the editor holds the document", "condition": "an editable text object holding the first line of the document", "found": editable[:8]}]
    return []


def save_status_problems(tree, names, key):
    """The save status readable as the `save.*` string for `key`."""
    wanted = names[key]
    for node in walk(tree):
        if wanted in (node.get("name") or "") or wanted in (node.get("text") or ""):
            return []
    others = [names[k] for k in ("save.clean", "save.dirty", "save.scheduled", "save.saving", "save.saved", "save.failed", "save.conflict")]
    seen = sorted({s for n in walk(tree) for s in others if s in (n.get("name") or "") or s in (n.get("text") or "")})
    return [{"step": "the save status", "condition": f"the status reads {wanted!r} ({key})", "found": seen}]


def notices_problems(tree):
    """The notices area exposed as a live region, even when empty."""
    live = [n for n in walk(tree) if (n.get("attrs") or {}).get("container-live") or (n.get("attrs") or {}).get("live")]
    if not live:
        return [{"step": "the notices area", "condition": "a live region is in the tree while no notice is showing", "found": nearest(tree, "status")}]
    return []


def settings_problems(tree, names):
    problems = []
    step = "the Settings dialog"
    _need(tree, step, "region", names["settings.title"], problems, "settings.title")
    _need(tree, step, "combobox", names["settings.language"], problems, "settings.language")
    _need(tree, step, "button", names["settings.close"], problems, "settings.close")
    return problems


def form_problems(tree, names):
    problems = []
    step = "Form Mode"
    for key in ("block.kind.heading", "block.kind.paragraph"):
        _need(tree, step, "textbox", names[key], problems, key)
    _need(tree, step, "button", names["form.undo"], problems, "form.undo")
    _need(tree, step, "button", names["form.redo"], problems, "form.redo")
    if not find_all(tree, "table"):
        problems.append({"step": step, "condition": "the table as a table", "found": nearest(tree, "table")})
    return problems


def first_problem(problems):
    """Raise the first problem as an `Anomaly`; do nothing for none."""
    if problems:
        p = problems[0]
        raise Anomaly(p["step"], p["condition"], p["found"])


def truncate(tree, max_depth=14, max_children=60):
    """A bounded copy of `tree` for a dump: depth and breadth limited, with the
    cut marked, so an artifact stays small and never silently drops a branch."""

    def cut(node, depth):
        out = {k: v for k, v in node.items() if k != "children"}
        children = node.get("children", [])
        if depth >= max_depth and children:
            out["children"] = [{"truncated": f"{len(children)} children below depth {max_depth}"}]
        else:
            out["children"] = [cut(c, depth + 1) for c in children[:max_children]]
            if len(children) > max_children:
                out["children"].append({"truncated": f"{len(children) - max_children} more children"})
        return out

    return cut(tree, 0)


def abort_report(anomaly, tree, steps_done):
    """The report written when a step fails: which step, what was required, what
    was found, how far the run got, and a truncated tree. No document text is in
    it beyond what is in the tree's own names and text."""
    return {
        "verdict": "ABORT",
        "step": anomaly.step,
        "condition": anomaly.condition,
        "found": anomaly.found,
        "steps_done": list(steps_done),
        "tree": truncate(tree) if tree else None,
    }


def render_report(report):
    """The lines printed to the log for a verdict."""
    if report["verdict"] != "ABORT":
        return [f"a11y observer PASSED: {', '.join(report['steps_done'])}"]
    found = json.dumps(report["found"], ensure_ascii=False)
    return [
        f"a11y observer ABORT at step {report['step']!r}",
        f"  required: {report['condition']}",
        f"  found:    {found[:1500]}",
        f"  done before: {', '.join(report['steps_done']) or 'nothing'}",
    ]
