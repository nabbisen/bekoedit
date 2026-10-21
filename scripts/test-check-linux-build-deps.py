#!/usr/bin/env python3
"""Self-test for scripts/check-linux-build-deps.py.

The real workflows pass. Then, in a throwaway copy of `.github` and `scripts`,
each rule is broken in exactly one way and must fail naming the job (or the
script), and a pure reformat of a passing step must still pass. Nothing in the
real tree is touched.

Usage: python3 scripts/test-check-linux-build-deps.py
"""

import pathlib
import shutil
import subprocess
import sys
import tempfile

import yaml

REPO = pathlib.Path(__file__).resolve().parent.parent
CHECK = REPO / "scripts" / "check-linux-build-deps.py"
SHARED_CALL = "bash scripts/install-linux-build-deps.sh"
ARCH_CALL = "bash scripts/install-arch-build-deps.sh"

failures = 0
cases = 0


def fresh_tree(work):
    root = pathlib.Path(tempfile.mkdtemp(dir=work))
    shutil.copytree(REPO / ".github", root / ".github")
    shutil.copytree(REPO / "scripts", root / "scripts")
    return root


def edit_workflow(root, name, change):
    path = root / ".github" / "workflows" / name
    document = yaml.safe_load(path.read_text())
    change(document["jobs"])
    path.write_text(yaml.safe_dump(document, sort_keys=False, width=10_000))


def run_check(root):
    done = subprocess.run(
        [sys.executable, str(CHECK), "--root", str(root)],
        capture_output=True,
        text=True,
    )
    return done.returncode, done.stdout + done.stderr


def expect(name, root, passes, mention=None):
    global failures, cases
    cases += 1
    code, output = run_check(root)
    good = (code == 0) if passes else (code != 0 and mention in output)
    if not good:
        failures += 1
        print(f"FAIL {name}: exit {code}, wanted {'pass' if passes else f'a failure naming {mention!r}'}")
        print(output)
    else:
        print(f"ok   {name}")


def call_index(steps):
    return next(i for i, s in enumerate(steps) if SHARED_CALL in str(s.get("run", "")))


def arch_call_index(steps):
    return next(i for i, s in enumerate(steps) if ARCH_CALL in str(s.get("run", "")))


def arch_cases(work):
    """RFC-045 slice 3a: the Arch container job, and raw pacman installs."""
    root = fresh_tree(work)
    def drop_arch_call(jobs):
        steps = jobs["build-arch"]["steps"]
        del steps[arch_call_index(steps)]
    edit_workflow(root, "arch-build.yml", drop_arch_call)
    expect("an Arch job without the Arch script fails, naming the job and the script", root, False,
           "arch-build.yml / build-arch] builds or publishes bekoedit on Linux (step 'Build native target') "
           "but never runs scripts/install-arch-build-deps.sh")

    root = fresh_tree(work)
    def apt_instead(jobs):
        steps = jobs["build-arch"]["steps"]
        steps[arch_call_index(steps)]["run"] = SHARED_CALL
    edit_workflow(root, "arch-build.yml", apt_instead)
    expect("an Arch job running the apt script instead of the Arch one fails", root, False,
           "arch-build.yml / build-arch] builds bekoedit in an Arch container but runs "
           "scripts/install-linux-build-deps.sh")

    root = fresh_tree(work)
    def both(jobs):
        steps = jobs["build-arch"]["steps"]
        steps.insert(0, {"name": "Apt too", "run": SHARED_CALL})
    edit_workflow(root, "arch-build.yml", both)
    expect("an Arch job that also runs the apt script does not pass by having the Arch one", root,
           False, "arch-build.yml / build-arch] builds bekoedit in an Arch container but runs")

    root = fresh_tree(work)
    def arch_after_build(jobs):
        steps = jobs["build-arch"]["steps"]
        steps.append(steps.pop(arch_call_index(steps)))
    edit_workflow(root, "arch-build.yml", arch_after_build)
    expect("the Arch script after the first build step fails, naming the job", root, False,
           "arch-build.yml / build-arch] runs scripts/install-arch-build-deps.sh only after")

    root = fresh_tree(work)
    def not_arch(jobs):
        jobs["build-arch"]["container"] = {"image": "ubuntu:24.04"}
    edit_workflow(root, "arch-build.yml", not_arch)
    expect("the same job in a non-Arch container is held to the apt script again", root, False,
           "arch-build.yml / build-arch] builds or publishes bekoedit on Linux")

    root = fresh_tree(work)
    def raw_pacman(jobs):
        jobs["audit"]["steps"].append(
            {"name": "Sneaky", "run": "pacman -S --noconfirm webkit2gtk-4.1"}
        )
    edit_workflow(root, "ci.yml", raw_pacman)
    expect("a raw pacman install of an Arch build package in a workflow step fails", root, False,
           "ci.yml / audit] step 'Sneaky' installs webkit2gtk-4.1 with a raw pacman install")

    root = fresh_tree(work)
    def raw_pacman_in_sh(jobs):
        jobs["audit"]["steps"].append(
            {"name": "Sneaky", "run": "docker run archlinux sh -c \"pacman -Syu --noconfirm git\""}
        )
    edit_workflow(root, "ci.yml", raw_pacman_in_sh)
    expect("the same, inside sh -c", root, False, "ci.yml / audit] step 'Sneaky' installs git")

    root = fresh_tree(work)
    def pacman_query(jobs):
        jobs["audit"]["steps"].append(
            {"name": "Query", "run": "pacman -Q webkit2gtk-4.1 || true"}
        )
    edit_workflow(root, "ci.yml", pacman_query)
    expect("a pacman query, which installs nothing, passes", root, True)

    root = fresh_tree(work)
    (root / "scripts" / "stray-arch.sh").write_text(
        "#!/bin/sh\npacman -Syu --noconfirm --needed webkit2gtk-4.1 xdotool\n"
    )
    expect("a raw pacman install in another script fails, naming it", root, False,
           "scripts/stray-arch.sh] installs webkit2gtk-4.1, xdotool with a raw pacman install")

    root = fresh_tree(work)
    harness = root / "scripts" / "check-linux-portability-containers.sh"
    expect("the portability harness's own runtime install, inside docker run, is the one exception",
           root, True)
    harness.rename(root / "scripts" / "renamed-harness.sh")
    expect("the exception is by name: the same lines in another script fail", root, False,
           "scripts/renamed-harness.sh] installs")

    root = fresh_tree(work)
    arch_script = root / "scripts" / "install-arch-build-deps.sh"
    arch_script.write_text(arch_script.read_text().replace("BUILD_DEPS=(", "ALSO_DEPS=("))
    expect("an Arch script with no BUILD_DEPS list fails", root, False, "has no BUILD_DEPS")

    root = fresh_tree(work)
    (root / "scripts" / "install-arch-build-deps.sh").unlink()
    expect("a missing Arch script fails", root, False, "install-arch-build-deps.sh is missing")


def main():
    with tempfile.TemporaryDirectory() as work:
        expect("the real workflows pass", REPO, True)

        root = fresh_tree(work)
        expect("an unmodified copy passes", root, True)

        root = fresh_tree(work)
        def drop_publish_call(jobs):
            steps = jobs["publish-crates"]["steps"]
            del steps[call_index(steps)]
        edit_workflow(root, "release.yml", drop_publish_call)
        expect("publish-crates without the shared call fails, naming the job", root, False,
               "release.yml / publish-crates")

        root = fresh_tree(work)
        def raw_install(jobs):
            jobs["audit"]["steps"].append(
                {"name": "Sneaky", "run": "sudo apt-get install -y libgtk-3-dev"}
            )
        edit_workflow(root, "ci.yml", raw_install)
        expect("a raw apt-get install of a build library in another job fails, naming it", root,
               False, "ci.yml / audit")

        root = fresh_tree(work)
        def raw_install_continued(jobs):
            jobs["audit"]["steps"].append(
                {"name": "Sneaky", "run": "sudo apt-get install -y \\\n  xvfb libxdo-dev"}
            )
        edit_workflow(root, "ci.yml", raw_install_continued)
        expect("the same, with a backslash continuation", root, False, "ci.yml / audit")

        root = fresh_tree(work)
        def call_after_build(jobs):
            steps = jobs["lint"]["steps"]
            steps.append(steps.pop(call_index(steps)))
        edit_workflow(root, "ci.yml", call_after_build)
        expect("the call after the first bekoedit build step fails, naming the job", root, False,
               "ci.yml / lint")

        root = fresh_tree(work)
        def call_off_linux(jobs):
            steps = jobs["publish-crates"]["steps"]
            steps[call_index(steps)]["if"] = "runner.os == 'macOS'"
        edit_workflow(root, "release.yml", call_off_linux)
        expect("a call that skips Linux fails, naming the job", root, False,
               "release.yml / publish-crates")

        root = fresh_tree(work)
        def reformat(jobs):
            steps = jobs["publish-crates"]["steps"]
            i = call_index(steps)
            # A folded scalar over two lines, the call spelled with ./ -- the
            # same command, written the way a grep for one line would miss it.
            steps[i] = {
                "name": "Install Linux build dependencies",
                "run": "bash \\\n  ./scripts/install-linux-build-deps.sh\n",
            }
        edit_workflow(root, "release.yml", reformat)
        expect("a reformatted but equivalent call still passes", root, True)

        root = fresh_tree(work)
        (root / "scripts" / "stray.sh").write_text(
            "#!/bin/sh\nsudo apt-get install -y libwebkit2gtk-4.1-dev\n"
        )
        expect("a raw install in another script fails, naming it", root, False,
               "scripts/stray.sh")

        root = fresh_tree(work)
        shared = root / "scripts" / "install-linux-build-deps.sh"
        shared.write_text(shared.read_text().replace("libxdo-dev", "libxdo"))
        expect("a shared script that drops a library fails, naming it", root, False, "libxdo-dev")

        root = fresh_tree(work)
        (root / "scripts" / "install-linux-build-deps.sh").unlink()
        expect("a missing shared script fails", root, False, "is missing")

        arch_cases(work)

    print(f"test-check-linux-build-deps: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
