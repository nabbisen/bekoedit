#!/usr/bin/env python3
"""The accessibility-tree observer (RFC-050 slice 1, level 3): what a screen
reader is given by the running bekoedit, read through AT-SPI.

    python3 scripts/a11y-observer.py --pid PID --profile DIR --out DIR

Run only by `scripts/a11y-tree-run.sh` (CI, under Xvfb, in a D-Bus session with the
AT-SPI bus up). Never on a developer machine: connecting to a live accessibility
bus would read the real desktop.

It acts only through AT-SPI: the Action interface (press a row, a tab, a button, a
menu item) and the EditableText interface (insert one marker). It sends no click
and no keystroke. The app, in `--webview-release-checks ... a11y_tree`, writes
`a11y-expected.json` (every expected name, looked up from `i18n.rs`), and compares
`note.md` on disk, byte for byte, after the Save this observer presses.

Everything with a decision in it is in `a11y_tree.py` and `a11y_steps.py`, tested
without a display. This file is the thin AT-SPI backend and the files around a run:
  PROFILE/a11y-expected.json     read (written by the app)
  PROFILE/a11y-observer-done     written last on success (the app waits for it)
  PROFILE/a11y-observer-abort    written on the first anomaly, first line the reason
  OUT/a11y-observer-report.json  the verdict, the steps done, a bounded tree dump
Exit code 0 on success, 1 on an abort.
"""

import argparse
import json
import pathlib
import sys
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import a11y_steps  # noqa: E402
import a11y_tree as T  # noqa: E402

MAX_NODES = 5000
MAX_DEPTH = 40
APP_SECONDS = 90
EXPECTED_SECONDS = 120


class AtspiBackend:
    """Reads the tree into plain dicts (with a `ref`) and acts by ref."""

    def __init__(self, Atspi, app):
        self.Atspi = Atspi
        self.app = app
        self.table = {}

    def snapshot(self):
        self.table = {}
        count = [0]

        def read(accessible, depth):
            ref = len(self.table)
            self.table[ref] = accessible
            count[0] += 1
            node = {"ref": ref, "role": "", "name": "", "states": [], "actions": [], "attrs": {}, "text": None, "children": []}
            try:
                node["role"] = accessible.get_role_name()
                node["name"] = accessible.get_name() or ""
                node["states"] = sorted(state.value_nick.replace("-", " ") for state in accessible.get_state_set().get_states())
                node["attrs"] = dict(accessible.get_attributes() or {})
                actions = accessible.get_action_iface()
                if actions is not None:
                    node["actions"] = [self.Atspi.Action.get_action_name(actions, i) for i in range(self.Atspi.Action.get_n_actions(actions))]
                text = accessible.get_text_iface()
                if text is not None:
                    node["text"] = self.Atspi.Text.get_text(text, 0, self.Atspi.Text.get_character_count(text))
            except Exception as error:  # a node that vanished mid-read is data, not a crash
                node["read_error"] = repr(error)
            if depth < MAX_DEPTH:
                try:
                    for i in range(accessible.get_child_count()):
                        if count[0] >= MAX_NODES:
                            node["children"].append({"truncated": f"stopped at {MAX_NODES} nodes"})
                            break
                        child = accessible.get_child_at_index(i)
                        if child is not None:
                            node["children"].append(read(child, depth + 1))
                except Exception as error:
                    node["read_error"] = repr(error)
            return node

        return read(self.app, 0)

    def act(self, ref, action):
        iface = self.table[ref].get_action_iface()
        names = [self.Atspi.Action.get_action_name(iface, i) for i in range(self.Atspi.Action.get_n_actions(iface))]
        if not self.Atspi.Action.do_action(iface, names.index(action)):
            raise T.Anomaly("act", f"the {action!r} action reports success", {"actions": names})

    def insert_text(self, ref, at, text):
        iface = self.table[ref].get_editable_text_iface()
        if iface is None:
            raise T.Anomaly("insert text", "the editor offers the EditableText interface", {"role": self.table[ref].get_role_name()})
        if not self.Atspi.EditableText.insert_text(iface, at, text, len(text.encode("utf-8"))):
            raise T.Anomaly("insert text", "EditableText.insert_text reports success", {"at": at, "text": text})


def applications(Atspi):
    desktop = Atspi.get_desktop(0)
    found = []
    for i in range(desktop.get_child_count()):
        accessible = desktop.get_child_at_index(i)
        if accessible is not None:
            found.append({"name": accessible.get_name(), "pid": accessible.get_process_id(), "accessible": accessible})
    return found


def find_app(Atspi, pid):
    """Wait, bounded, for the launched process to appear on the bus."""
    deadline = time.monotonic() + APP_SECONDS
    while True:
        try:
            return T.pick_application(applications(Atspi), pid)["accessible"]
        except T.Anomaly:
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.5)


def wait_for_expected(path):
    deadline = time.monotonic() + EXPECTED_SECONDS
    while not path.exists():
        if time.monotonic() >= deadline:
            raise T.Anomaly("read the expectations", f"{path.name} written by the app within {EXPECTED_SECONDS}s", {"dir": sorted(p.name for p in path.parent.iterdir())})
        time.sleep(0.25)
    return json.loads(path.read_text(encoding="utf-8"))


def finish(profile, out, report, ok):
    out.mkdir(parents=True, exist_ok=True)
    (out / "a11y-observer-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
    for line in T.render_report(report):
        print(line)
    # The app creates its profile directory itself and refuses one that exists,
    # so the observer never creates it: if the app is gone, there is nobody to tell.
    if profile.is_dir():
        if ok:
            (profile / "a11y-observer-done").write_text("done\n")
        else:
            (profile / "a11y-observer-abort").write_text(f"{report['step']}: {report['condition']}\n")
    return 0 if ok else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--profile", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args()

    observer = None
    try:
        import gi

        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi

        Atspi.init()
        expected = wait_for_expected(args.profile / "a11y-expected.json")
        missing = [key for key in T.REQUIRED_KEYS if not expected.get("names", {}).get(key)]
        if missing:
            raise T.Anomaly("read the expectations", "every required name is present", {"missing": missing})
        backend = AtspiBackend(Atspi, find_app(Atspi, args.pid))
        observer = a11y_steps.Observer(backend, expected)
        observer.run()
        report = {"verdict": "PASS", "steps_done": observer.steps_done, "tree": T.truncate(observer.last_tree)}
        return finish(args.profile, args.out, report, True)
    except T.Anomaly as anomaly:
        tree = observer.last_tree if observer else None
        done = observer.steps_done if observer else []
        return finish(args.profile, args.out, T.abort_report(anomaly, tree, done), False)
    except Exception as error:  # the observer's own failure is an abort too, so the app does not wait out its deadline
        anomaly = T.Anomaly("observer", f"the observer runs ({type(error).__name__})", repr(error))
        tree = observer.last_tree if observer else None
        return finish(args.profile, args.out, T.abort_report(anomaly, tree, observer.steps_done if observer else []), False)


if __name__ == "__main__":
    sys.exit(main())
