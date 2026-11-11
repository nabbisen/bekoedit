#!/usr/bin/env python3
"""Unit tests for scripts/a11y_tree.py (RFC-050 slice 1, level 3's pure part).

They run on recorded trees: plain data in the shape the observer reads from
AT-SPI, with no display and no `gi`. They cover matching by role and exact name,
picking the application by process id, a missing control, a blank name, and what
an abort report holds. The names are the Japanese strings the app would write
into its expectations file; here they are fixed samples, because the unit tests
check the observer's logic, and which strings the app writes is checked by the
app's own tests.

Usage: python3 scripts/test-a11y-observer.py
"""

import copy
import json
import pathlib
import re
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import a11y_tree as t  # noqa: E402

NAMES = {
    "app.title": "bekoedit",
    "explorer.label": "ワークスペースエクスプローラ",
    "explorer.tree_label": "ファイル",
    "explorer.new_file": "新規ファイル",
    "mode.text": "テキスト",
    "mode.form": "フォーム",
    "mode.preview": "プレビュー",
    "editor.mode_label": "編集モード",
    "editor.save": "保存",
    "menu.app": "アプリケーションメニュー",
    "settings.title": "設定",
    "settings.language": "言語",
    "settings.close": "閉じる",
    "block.kind.heading": "見出し",
    "block.kind.paragraph": "段落",
    "block.kind.table": "表",
    "form.undo": "元に戻す",
    "form.redo": "やり直し",
    "save.clean": "すべての変更を保存済み",
    "save.dirty": "未保存の変更",
    "save.scheduled": "自動保存を予定",
    "save.saving": "保存中…",
    "save.saved": "保存しました",
    "save.failed": "保存に失敗",
    "save.conflict": "競合の解決が必要",
    "toast.dismiss": "通知を閉じる",
}
FILES = ["note.md", "other.md"]
DOC = "# Title\n\nA paragraph.\n"
PID = 4242

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


def n(role, name="", children=(), **extra):
    return {"role": role, "name": name, "states": extra.pop("states", []), "actions": extra.pop("actions", []),
            "attrs": extra.pop("attrs", {}), "text": extra.pop("text", None), "children": list(children), **extra}


def shell(names=NAMES, selected="mode.text", tabs=("mode.text", "mode.form", "mode.preview"), files=FILES, editor_text=DOC):
    return n("application", "bekoedit", pid=PID, children=[
        n("frame", "bekoedit", children=[
            n("landmark", names["explorer.label"], children=[
                n("push button", names["explorer.new_file"], actions=["click"]),
                n("tree", names["explorer.tree_label"], children=[
                    n("tree item", f, actions=["click"], states=["selectable"]) for f in files]),
            ]),
            n("page tab list", names["editor.mode_label"], children=[
                n("page tab", names[k], actions=["click"], states=["selected"] if k == selected else []) for k in tabs]),
            n("push button", names["editor.save"], actions=["click"]),
            n("push button", names["menu.app"], actions=["click"]),
            n("entry", "", states=["editable", "multi line"], text=editor_text) if editor_text is not None else n("section"),
            n("status bar", "", text=names["save.clean"]),
            n("section", "", attrs={"container-live": "polite"}),
        ])])


def problems_text(problems):
    return json.dumps(problems, ensure_ascii=False)


def main():
    # --- matching by role and exact name ---
    tree = shell()
    check("a control is found by AT-SPI role and exact name", len(t.find_all(tree, "button", NAMES["editor.save"])) == 1)
    check("the same name with another role does not match", t.find_all(tree, "tab", NAMES["editor.save"]) == [])
    check("a name that differs by one character does not match", t.find_all(tree, "button", NAMES["editor.save"] + " ") == [])
    check("a role is matched through its AT-SPI aliases (button is a push button)", t.role_matches("button", "push button"))
    check("an unknown aria role matches only itself", t.role_matches("gizmo", "gizmo") and not t.role_matches("gizmo", "push button"))

    # --- the whole shell present ---
    check("a complete shell has no problems", t.shell_problems(tree, NAMES, FILES) == [], problems_text(t.shell_problems(tree, NAMES, FILES)))

    # --- a missing control, a blank name ---
    without_save = copy.deepcopy(tree)
    frame = without_save["children"][0]
    frame["children"] = [c for c in frame["children"] if c.get("name") != NAMES["editor.save"]]
    p = t.shell_problems(without_save, NAMES, FILES)
    check("a missing control is reported with its role and name", len(p) == 1 and NAMES["editor.save"] in p[0]["condition"] and "button" in p[0]["condition"], problems_text(p))
    check("and with the nearest buttons that are there", p and NAMES["menu.app"] in problems_text(p[0]["found"]))
    blank = copy.deepcopy(tree)
    for node in t.walk(blank):
        if node.get("name") == NAMES["editor.save"]:
            node["name"] = ""
    p = t.shell_problems(blank, NAMES, FILES)
    check("a blank accessible name is a missing control, not a pass", len(p) == 1 and NAMES["editor.save"] in p[0]["condition"], problems_text(p))
    english = shell(names={**NAMES, "editor.save": "Save"})
    check("the wrong language's name is a missing control", any(NAMES["editor.save"] in x["condition"] for x in t.shell_problems(english, NAMES, FILES)))
    norows = shell(files=["note.md"])
    check("a seeded file with no row is reported by file name", any("other.md" in x["condition"] for x in t.shell_problems(norows, NAMES, FILES)))
    noaction = copy.deepcopy(tree)
    for node in t.walk(noaction):
        if node.get("name") == "note.md":
            node["actions"] = []
    check("a row with no action to open it is reported", any("action to open it" in x["condition"] for x in t.shell_problems(noaction, NAMES, FILES)))

    nosave = copy.deepcopy(tree)
    for node in t.walk(nosave):
        if node.get("name") == NAMES["editor.save"]:
            node["actions"] = []
    p = t.shell_problems(nosave, NAMES, FILES)
    check("a Save control with no action is reported as that, and nothing else", len(p) == 1 and "Save control has an action" in p[0]["condition"], problems_text(p))

    # --- the tabs: exactly one selected, the right one ---
    check("two selected tabs are reported", any("exactly one tab selected" in x["condition"] for x in t.shell_problems(
        (lambda x: (x["children"][0]["children"][1]["children"][1]["states"].append("selected"), x)[1])(copy.deepcopy(tree)), NAMES, FILES)))
    check("the wrong tab selected is reported", any("exactly one tab selected" in x["condition"] for x in t.shell_problems(shell(selected="mode.form"), NAMES, FILES)))
    check("no tab selected is reported", any("exactly one tab selected" in x["condition"] for x in t.shell_problems(shell(selected="none"), NAMES, FILES)))

    # --- picking the application by pid ---
    apps = [n("application", "bekoedit", pid=111), n("application", "bekoedit", pid=PID), n("application", "other", pid=333)]
    check("the application with the launched pid is the one picked, among others named alike", t.pick_application(apps, PID)["pid"] == PID)
    for label, pool in (("none has the pid", [n("application", "bekoedit", pid=1)]), ("two have the pid", [n("application", "a", pid=PID), n("application", "b", pid=PID)])):
        try:
            t.pick_application(pool, PID)
            check(f"{label}: refused", False)
        except t.Anomaly as anomaly:
            check(f"{label}: refused, naming the pids seen", f"process id {PID}" in anomaly.condition and "applications" in anomaly.found)
    picked_first = [a for a in apps if a["name"] == "bekoedit"][0]
    check("the first application named bekoedit is NOT the one picked (a different pid)", picked_first["pid"] != t.pick_application(apps, PID)["pid"])

    # --- the editor ---
    check("the editor is the editable text object holding the document", t.editor_problems(tree, DOC) == [])
    check("the editor's name is whatever it is, and is reported", t.editor_node(tree, DOC)["name"] == "")
    noeditor = shell(editor_text="# Something else\n")
    p = t.editor_problems(noeditor, DOC)
    check("an editor holding another document is reported with the editable objects found", len(p) == 1 and "editable text object" in p[0]["condition"] and p[0]["found"], problems_text(p))
    check("no editor at all is reported", len(t.editor_problems(shell(editor_text=None), DOC)) == 1)
    notediting = shell()
    for node in t.walk(notediting):
        node["states"] = [s for s in node["states"] if s != "editable"]
    check("a text object that is not editable is not the editor", len(t.editor_problems(notediting, DOC)) == 1)

    # --- the save status ---
    check("the status reads as the save string for the key", t.save_status_problems(tree, NAMES, "save.clean") == [])
    p = t.save_status_problems(tree, NAMES, "save.saved")
    check("another save string is reported with the ones that are showing", len(p) == 1 and NAMES["save.clean"] in problems_text(p), problems_text(p))

    # --- the notices area ---
    check("a live region present while no notice shows passes", t.notices_problems(tree) == [])
    nolive = copy.deepcopy(tree)
    for node in t.walk(nolive):
        node["attrs"] = {}
    check("no live region in the tree is reported", len(t.notices_problems(nolive)) == 1)

    # --- settings and form ---
    settings = n("application", "bekoedit", pid=PID, children=[n("landmark", NAMES["settings.title"]), n("combo box", NAMES["settings.language"]), n("push button", NAMES["settings.close"])])
    settings["children"][0]["role"] = "section"
    check("the Settings dialog's region, language and close are matched", t.settings_problems(settings, NAMES) == [], problems_text(t.settings_problems(settings, NAMES)))
    check("a Settings dialog without Close is reported", len(t.settings_problems(n("application", "x", children=settings["children"][:2]), NAMES)) == 1)
    form = n("application", "bekoedit", pid=PID, children=[n("entry", NAMES["block.kind.heading"]), n("entry", NAMES["block.kind.paragraph"]), n("push button", NAMES["form.undo"]), n("push button", NAMES["form.redo"]), n("table", "")])
    check("Form Mode's fields, history buttons and table are matched", t.form_problems(form, NAMES) == [])
    check("Form Mode without the table is reported", any("table" in x["condition"] for x in t.form_problems(n("application", "x", children=form["children"][:4]), NAMES)))

    # --- the first problem aborts; the report holds what the handoff asks ---
    try:
        t.first_problem(t.shell_problems(without_save, NAMES, FILES))
        check("a problem raises an anomaly", False)
    except t.Anomaly as anomaly:
        report = t.abort_report(anomaly, without_save, ["pick the application", "wait for the shell"])
        lines = t.render_report(report)
        check("the abort report names the step, the condition, what was found and how far it got",
              report["verdict"] == "ABORT" and report["step"] == "the shell is in the tree" and NAMES["editor.save"] in report["condition"]
              and report["found"] and report["steps_done"] == ["pick the application", "wait for the shell"] and report["tree"]["role"] == "application")
        check("and prints them", len(lines) == 4 and "required:" in lines[1] and "found:" in lines[2] and "done before: pick the application" in lines[3], str(lines))
    t.first_problem([])
    check("no problem raises nothing", True)

    # --- the dump is bounded and marks its cuts ---
    deep = n("a", children=[n("b", children=[n("c", children=[n("d")])])])
    cut = t.truncate(deep, max_depth=1)
    check("a dump cuts below its depth and says so", "truncated" in json.dumps(cut) and "d" not in [x.get("role") for x in t.walk(cut)])
    wide = n("a", children=[n("x", str(i)) for i in range(100)])
    cut = t.truncate(wide, max_children=10)
    check("a dump keeps the first children and counts the rest", len(cut["children"]) == 11 and "90 more" in cut["children"][-1]["truncated"])
    fixture = json.loads((HERE / "testdata" / "a11y" / "tree-ja.json").read_text())
    check("the recorded fixture loads and passes the shell check", t.shell_problems(fixture, NAMES, FILES) == [], problems_text(t.shell_problems(fixture, NAMES, FILES)))

    # --- the observer and the app agree on the expectation keys ---
    app_source = (HERE.parent / "crates/bekoedit-app/src/webview_smoke/release_checks/a11y_tree.rs").read_text()
    app_keys = set(re.findall(r'"([a-z_]+\.[a-z_.]+)"', app_source.split("A11Y_KEYS")[1].split("];")[0]))
    check("every key the observer requires is written by the app", set(t.REQUIRED_KEYS) <= app_keys, f"missing: {sorted(set(t.REQUIRED_KEYS) - app_keys)}")
    check("every key the tests sample is a required key", set(NAMES) == set(t.REQUIRED_KEYS), f"differ: {sorted(set(NAMES) ^ set(t.REQUIRED_KEYS))}")

    print(f"test-a11y-observer: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
