#!/usr/bin/env python3
"""Unit tests for scripts/a11y_steps.py (RFC-050 slice 1, level 3's step sequence).

The sequence runs against a *fake backend*: a small model of the app that builds
a tree from its state and changes state only when the observer presses a control
through an action or inserts text through EditableText. No display, no `gi`, no
clock (the fake clock advances when the observer sleeps). The cases cover the
whole run, a shell that appears late, a shell that never appears, an action that
is missing, an edit the app does not register, and a dialog that does not close.

Usage: python3 scripts/test-a11y-steps.py
"""

import json
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import a11y_steps as s  # noqa: E402
import a11y_tree as t  # noqa: E402

NAMES = {
    "app.title": "bekoedit", "explorer.label": "エクスプローラ", "explorer.tree_label": "ファイル",
    "explorer.new_file": "新規", "mode.text": "テキスト", "mode.form": "フォーム", "mode.preview": "プレビュー",
    "editor.mode_label": "モード", "editor.save": "保存", "menu.app": "メニュー", "settings.title": "設定",
    "settings.language": "言語", "settings.close": "閉じる", "block.kind.heading": "見出し",
    "block.kind.paragraph": "段落", "block.kind.table": "表", "form.undo": "元に戻す", "form.redo": "やり直し",
    "save.clean": "保存済み", "save.dirty": "未保存", "save.scheduled": "予定", "save.saving": "保存中",
    "save.saved": "保存しました", "save.failed": "失敗", "save.conflict": "競合", "toast.dismiss": "閉じる通知",
}
EXPECTED = {"names": NAMES, "files": ["note.md", "other.md"], "marker": "ZQ7", "document": "# Title\n\nbody\n"}

failures = 0
cases = 0


def check(name, ok, detail=""):
    global failures, cases
    cases += 1
    if ok:
        print(f"ok   {name}")
    else:
        failures += 1
        print(f"FAIL {name} {detail}")


class Fake:
    """A model of the app. `break_*` flags are the faults the cases inject."""

    def __init__(self, late=0, never=False, save_without_action=False, ignore_edit=False, settings_sticks=False):
        self.polls = 0
        self.late, self.never = late, never
        self.save_without_action, self.ignore_edit, self.settings_sticks = save_without_action, ignore_edit, settings_sticks
        self.file_open = False
        self.text = EXPECTED["document"]
        self.status = "save.clean"
        self.mode = "mode.text"
        self.menu = False
        self.settings = False
        self.disk = EXPECTED["document"]
        self.log = []
        self.refs = {}

    def node(self, role, name="", children=(), kind=None, **extra):
        ref = len(self.refs)
        self.refs[ref] = kind
        return {"role": role, "name": name, "states": extra.pop("states", []), "actions": extra.pop("actions", []),
                "attrs": extra.pop("attrs", {}), "text": extra.pop("text", None), "ref": ref, "children": list(children), **extra}

    def snapshot(self):
        self.log.append("snapshot")
        self.polls += 1
        self.refs = {}
        n, node = NAMES, self.node
        if self.never or self.polls <= self.late:
            return node("application", "bekoedit", pid=1, children=[node("frame", "bekoedit")])
        form = []
        if self.mode == "mode.form":
            form = [node("entry", n["block.kind.heading"]), node("entry", n["block.kind.paragraph"]),
                    node("push button", n["form.undo"], actions=["click"]), node("push button", n["form.redo"], actions=["click"]),
                    node("table", "")]
        settings = []
        if self.settings:
            settings = [node("panel", n["settings.title"], children=[
                node("combo box", n["settings.language"]), node("push button", n["settings.close"], actions=["click"], kind="close")])]
        save_actions = [] if self.save_without_action and self.status == "save.dirty" else ["click"]
        shell = [
            node("landmark", n["explorer.label"], children=[
                node("push button", n["explorer.new_file"], actions=["click"]),
                node("tree", n["explorer.tree_label"], children=[
                    node("tree item", f, actions=["click"], kind="row") for f in EXPECTED["files"]])]),
            node("page tab list", n["editor.mode_label"], children=[
                node("page tab", n[k], actions=["click"], kind=k, states=["selected"] if k == self.mode else [])
                for k in ("mode.text", "mode.form", "mode.preview")]),
            node("push button", n["editor.save"], actions=save_actions, kind="save"),
            node("push button", n["menu.app"], actions=["click"], kind="menu"),
            node("status bar", "", text=n[self.status]),
            node("section", "", attrs={"container-live": "polite"}),
        ]
        if self.menu:
            shell.append(node("menu item", n["settings.title"], actions=["click"], kind="settings"))
        if self.file_open and self.mode == "mode.text":
            shell.append(node("entry", "", states=["editable", "multi line"], text=self.text, kind="editor"))
        return node("application", "bekoedit", pid=1, children=[node("frame", "bekoedit", children=shell + form + settings)])

    def act(self, ref, action):
        kind = self.refs[ref]
        self.log.append(f"act:{kind}:{action}")
        if kind == "row":
            self.file_open = True
        elif kind == "save":
            self.disk, self.status = self.text, "save.saved"
        elif kind in ("mode.form", "mode.text", "mode.preview"):
            self.mode = kind
        elif kind == "menu":
            self.menu = True
        elif kind == "settings":
            self.menu, self.settings = False, True
        elif kind == "close" and not self.settings_sticks:
            self.settings = False

    def insert_text(self, ref, at, text):
        self.log.append(f"insert:{self.refs[ref]}:{at}")
        if not self.ignore_edit:
            self.text, self.status = text + self.text, "save.dirty"


def run(fake):
    clock = {"now": 0.0}

    def sleep(seconds):
        clock["now"] += seconds

    obs = s.Observer(fake, EXPECTED, sleep=sleep, clock=lambda: clock["now"])
    try:
        return obs, obs.run(), None
    except t.Anomaly as anomaly:
        return obs, None, anomaly


def main():
    fake = Fake()
    obs, report, anomaly = run(fake)
    check("a whole run passes", anomaly is None and report and "PASSED" in report[0], anomaly and (anomaly.step, anomaly.condition, anomaly.found))
    check("it did every step", len(obs.steps_done) == 8, obs.steps_done)
    check("the file on disk is the marker then the original", fake.disk == "ZQ7" + EXPECTED["document"], fake.disk)
    check("the observer acted only through actions and EditableText",
          all(op == "snapshot" or op.startswith(("act:", "insert:")) for op in fake.log))
    check("it pressed Save, the Form tab, the menu, Settings and Close, in that order",
          [o for o in fake.log if o.startswith("act:") and o != "act:row:click"] ==
          ["act:save:click", "act:mode.form:click", "act:menu:click", "act:settings:click", "act:close:click"], fake.log)

    obs, report, anomaly = run(Fake(late=5))
    check("a shell that appears late passes", anomaly is None)

    obs, report, anomaly = run(Fake(never=True))
    check("a shell that never appears stops at the first step, after the load deadline",
          anomaly is not None and anomaly.step == "the shell is in the tree" and f"waited {s.LOAD_SECONDS}s" in anomaly.condition,
          anomaly and (anomaly.step, anomaly.condition))
    check("and nothing was done", obs.steps_done == [])

    fake = Fake(save_without_action=True)
    obs, report, anomaly = run(fake)
    check("a Save that has no action when the edit is made stops at the press, naming the actions found",
          anomaly is not None and anomaly.step == "press Save" and anomaly.found["actions"] == [], anomaly and (anomaly.step, anomaly.found))
    check("and the file on disk is untouched", fake.disk == EXPECTED["document"])

    obs, report, anomaly = run(Fake(ignore_edit=True))
    check("an edit the app does not register stops at the status, naming the status found",
          anomaly is not None and anomaly.step == "the save status", anomaly and (anomaly.step, anomaly.condition))

    obs, report, anomaly = run(Fake(settings_sticks=True))
    check("a Settings dialog that does not close stops there", anomaly is not None and anomaly.step == "close Settings", anomaly and (anomaly.step, anomaly.condition))
    check("and everything before it is recorded as done", len(obs.steps_done) == 6, obs.steps_done)

    fake = Fake()
    obs, _, _ = run(fake)
    dump = json.dumps(t.abort_report(t.Anomaly("x", "y", {}), obs.last_tree, obs.steps_done), ensure_ascii=False)
    check("an abort report made from the observer's last tree is JSON", '"verdict": "ABORT"' in dump)

    print(f"test-a11y-steps: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
