"""The step sequence of the accessibility-tree observer (RFC-050 slice 1, level 3).

`scripts/a11y-observer.py` supplies a *backend* that talks AT-SPI; this module is
the sequence of steps run against any backend, so that the order, the waits and the
aborts are unit-tested with a fake backend (`scripts/test-a11y-observer.py`), with
no display. A backend offers:

    snapshot()                 -> a tree (see a11y_tree.py), every node with a "ref"
    act(ref, action)           -> press the control through that AT-SPI action
    insert_text(ref, at, text) -> insert through the EditableText interface

The observer acts only through these. There is no click, no keystroke, no
synthetic input anywhere in it. Each step asserts what it needs and stops at the
first anomaly (`Anomaly`), with the tree it saw.
"""

import time

import a11y_tree as T

# Actions that mean "activate this control", in preference order. A control with
# none of them has no usable action: that is a finding, not something to work
# around with a click.
ACTIVATE = ("click", "press", "activate", "jump", "open")

LOAD_SECONDS = 60
STEP_SECONDS = 20
POLL_SECONDS = 0.25


class Observer:
    def __init__(self, backend, expected, sleep=time.sleep, clock=time.monotonic):
        self.backend = backend
        self.names = expected["names"]
        self.files = expected["files"]
        self.marker = expected["marker"]
        self.document = expected["document"]
        self.sleep = sleep
        self.clock = clock
        self.steps_done = []
        self.last_tree = None

    # -- helpers ---------------------------------------------------------

    def snapshot(self):
        self.last_tree = self.backend.snapshot()
        return self.last_tree

    def wait_until(self, step, seconds, problems_of):
        """Poll until `problems_of(tree)` is empty. This waits for the app to
        reach a state; it is not a retry of an action. On the deadline the last
        problems are the anomaly."""
        deadline = self.clock() + seconds
        while True:
            problems = problems_of(self.snapshot())
            if not problems:
                return self.last_tree
            if self.clock() >= deadline:
                first = problems[0]
                raise T.Anomaly(first["step"], first["condition"] + f" (waited {seconds}s)", first["found"])
            self.sleep(POLL_SECONDS)

    def activate(self, step, tree, aria_role, name):
        matches = T.find_all(tree, aria_role, name)
        if len(matches) != 1:
            raise T.Anomaly(step, f"exactly one {aria_role} named {name!r} to act on", {"found": len(matches), "near": T.nearest(tree, aria_role)})
        node = matches[0]
        chosen = next((a for a in ACTIVATE if a in node.get("actions", [])), None)
        if chosen is None:
            raise T.Anomaly(step, f"{aria_role} {name!r} has an action that activates it ({', '.join(ACTIVATE)})", {"actions": node.get("actions", []), "states": node.get("states", [])})
        self.backend.act(node["ref"], chosen)
        return chosen

    def done(self, label):
        self.steps_done.append(label)

    # -- the steps -------------------------------------------------------

    def run(self):
        n = self.names
        self.wait_until("the shell", LOAD_SECONDS, lambda t: T.shell_problems(t, n, self.files))
        self.done("the shell is in the tree")

        note = self.files[0]
        tree = self.last_tree
        self.activate("open the first file", tree, "treeitem", note)
        self.wait_until("the editor", STEP_SECONDS, lambda t: T.editor_problems(t, self.document))
        self.done(f"{note} opened through its row's action")

        editor = T.editor_node(self.last_tree, self.document)
        self.backend.insert_text(editor["ref"], 0, self.marker)
        self.wait_until("the edit", STEP_SECONDS, lambda t: self._status_one_of(t, ("save.dirty", "save.scheduled")))
        self.done("the marker inserted through EditableText; the status reads unsaved")

        self.activate("press Save", self.last_tree, "button", n["editor.save"])
        self.wait_until("the save", STEP_SECONDS, lambda t: T.save_status_problems(t, n, "save.saved"))
        self.done("Save pressed through its action; the status reads saved")

        self.activate("open Form Mode", self.last_tree, "tab", n["mode.form"])
        self.wait_until("Form Mode", STEP_SECONDS, lambda t: T.form_problems(t, n))
        self.done("Form Mode through the tab's action; its fields, history buttons and table are named")

        self.activate("open the app menu", self.last_tree, "button", n["menu.app"])
        self.wait_until("the app menu", STEP_SECONDS, lambda t: self._need_item(t, "menuitem", n["settings.title"]))
        self.activate("open Settings", self.last_tree, "menuitem", n["settings.title"])
        self.wait_until("Settings", STEP_SECONDS, lambda t: T.settings_problems(t, n))
        self.done("Settings through the menu; its region and controls are named")

        self.activate("close Settings", self.last_tree, "button", n["settings.close"])
        self.wait_until("Settings closed", STEP_SECONDS, lambda t: self._gone(t, "region", n["settings.title"]))
        self.done("Settings closed through its button's action")

        T.first_problem(T.notices_problems(self.snapshot()))
        self.done("the notices area is a live region")
        return T.render_report({"verdict": "PASS", "steps_done": self.steps_done})

    # -- small predicates ------------------------------------------------

    def _status_one_of(self, tree, keys):
        problems = [T.save_status_problems(tree, self.names, k) for k in keys]
        if any(not p for p in problems):
            return []
        return problems[0]

    def _need_item(self, tree, aria_role, name):
        if T.find_all(tree, aria_role, name):
            return []
        return [{"step": "the app menu", "condition": f"a {aria_role} named {name!r}", "found": T.nearest(tree, aria_role)}]

    def _gone(self, tree, aria_role, name):
        if not T.find_all(tree, aria_role, name):
            return []
        return [{"step": "close Settings", "condition": f"no {aria_role} named {name!r} after Close", "found": T.nearest(tree, aria_role)}]
