#!/usr/bin/env python3
"""Structure test for the accessibility-tree check (RFC-050 slice 1, level 3).

The check needs a real window and an accessibility bus, so it cannot run in `lint`;
what can be checked here is that it is wired the way its rules say:

  - the workflow is started by hand only (step B, the push trigger, is a later request);
  - the observer's own unit tests run before the build, and the run script runs after it;
  - the verdict and tree dump are uploaded even when the run failed (`if: always()`);
  - the scenario is not in ci.yml's release-checks loop (it needs the observer);
  - the run script refuses to run outside GitHub Actions, and never on a developer machine;
  - the observer sends no click and no keystroke.

Usage: python3 scripts/test-a11y-workflow.py
"""

import os
import pathlib
import re
import subprocess
import sys

import yaml

REPO = pathlib.Path(__file__).resolve().parent.parent
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


def main():
    workflow = yaml.safe_load((REPO / ".github" / "workflows" / "a11y-tree.yml").read_text())
    triggers = workflow.get("on", workflow.get(True))  # YAML reads a bare `on` as True
    check("it is started by hand only", list(triggers) == ["workflow_dispatch"], triggers)
    steps = workflow["jobs"]["a11y-tree"]["steps"]
    runs = [str(s.get("run", "")) for s in steps]

    def index(needle):
        return next((i for i, r in enumerate(runs) if needle in r), None)

    unit, build, run = index("test-a11y-observer.py"), index("cargo +1.88.0 build"), index("a11y-tree-run.sh")
    check("the observer's unit tests, the step tests, the build and the run are all there", None not in (unit, build, run), (unit, build, run))
    check("the unit tests run before the build, the run after it", unit < build < run, (unit, build, run))
    check("the step tests run in the same step as the observer tests", "test-a11y-steps.py" in runs[unit])
    check("the run uses the release binary", "target/release/bekoedit" in runs[run])
    upload = [s for s in steps if str(s.get("uses", "")).startswith("actions/upload-artifact@")]
    check("exactly one upload, pinned by hash", len(upload) == 1 and re.search(r"@[0-9a-f]{40}", upload[0]["uses"]) is not None)
    check("the upload runs even when the run failed", upload and upload[0].get("if") == "always()", upload and upload[0].get("if"))
    check("and it carries the output directory the run wrote", upload and "a11y-out" in str(upload[0]["with"]["path"]) and "a11y-out" in runs[run])
    check("the job has a time limit", "timeout-minutes" in workflow["jobs"]["a11y-tree"])
    check("nothing in the workflow is made non-blocking", all("continue-on-error" not in s for s in steps))

    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    loop = next(line for line in ci.splitlines() if "for scenario in reopen_usable" in line)
    check("the scenario is not in ci.yml's release-checks loop", "a11y_tree" not in loop)

    env = {k: v for k, v in os.environ.items() if k != "GITHUB_ACTIONS"}
    done = subprocess.run(["bash", str(REPO / "scripts" / "a11y-tree-run.sh"), "/bin/true", "/nonexistent"], capture_output=True, text=True, env=env)
    check("the run script refuses outside GitHub Actions", done.returncode == 2 and "only in GitHub Actions" in done.stderr, (done.returncode, done.stderr))

    observer = (REPO / "scripts" / "a11y-observer.py").read_text() + (REPO / "scripts" / "a11y_steps.py").read_text()
    code = "\n".join(line for line in observer.splitlines() if not line.lstrip().startswith("#"))
    forbidden = re.findall(r"xdotool|generate_mouse_event|generate_keyboard_event|DeviceEventController|pyautogui|xte\b|uinput", code)
    check("the observer sends no click and no keystroke", not forbidden, forbidden)

    print(f"test-a11y-workflow: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
