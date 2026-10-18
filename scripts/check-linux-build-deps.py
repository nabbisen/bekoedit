#!/usr/bin/env python3
"""Task 063: Linux build libraries come from one source.

`bekoedit` needs the GTK and WebKit development libraries to build, and
`cargo publish` verifies a crate by building it. At the 0.18.0 tag the
publish job lacked them and failed on the sixth crate. The libraries are now
installed by scripts/install-linux-build-deps.sh, and this check fails unless:

  1. every job that builds or publishes `bekoedit` on Linux runs that script
     before its first such step, and the step that runs it is not one
     that skips Linux;
  2. no other workflow step and no other script installs those packages with
     a raw `apt-get install`.

The workflows are parsed as YAML, so a reformat (a folded scalar, a flow
mapping, a reordered key) neither defeats the check nor trips it. A step's
`run` text is split like a shell would split it, with backslash continuations
joined and comments dropped, before it is read.

Usage: python3 scripts/check-linux-build-deps.py [--root DIR]
"""

import pathlib
import re
import shlex
import sys

import yaml

SHARED = "scripts/install-linux-build-deps.sh"
PACKAGES = ("libwebkit2gtk-4.1-dev", "libgtk-3-dev", "libxdo-dev")

# `cargo <verb> ...` that compiles the desktop app: it names the package, or
# the whole workspace. `-p bekoedit-fs` is another crate, and a bare
# `cargo test` runs the default members, which need no GUI libraries.
CARGO_VERBS = {"build", "test", "clippy", "check", "doc", "run", "bench", "publish"}
APP_SELECTORS = {"--workspace", "--all"}


def logical_lines(text):
    """The shell's view of a script: continuations joined, comments dropped."""
    joined = re.sub(r"\\\n[ \t]*", " ", text)
    for line in joined.splitlines():
        try:
            tokens = shlex.split(line, comments=True)
        except ValueError:
            tokens = line.split()
        if tokens:
            yield tokens


def builds_app(tokens):
    if "cargo" not in tokens:
        return False
    rest = tokens[tokens.index("cargo") + 1 :]
    rest = [t for t in rest if not t.startswith("+")]
    if not rest or rest[0] not in CARGO_VERBS:
        return False
    if any(t in APP_SELECTORS for t in rest):
        return True
    return any(a in ("-p", "--package") and b == "bekoedit" for a, b in zip(rest, rest[1:]))


def step_builds_app(run):
    return any(builds_app(t) for t in logical_lines(run)) or any(
        "scripts/publish-crates.sh" in t for tokens in logical_lines(run) for t in tokens
    )


def runs_shared(run):
    return any(
        t.removeprefix("./") == SHARED for tokens in logical_lines(run) for t in tokens
    )


def raw_install(run):
    """The packages a raw apt install in `run` names, if any."""
    found = []
    for tokens in logical_lines(run):
        if "install" in tokens and any(t in ("apt", "apt-get") for t in tokens):
            found += [t for t in tokens if t in PACKAGES]
    return found


def runs_on_linux(job):
    runs_on = str(job.get("runs-on", ""))
    values = [runs_on]
    for name in re.findall(r"\$\{\{\s*matrix\.(\w+)\s*\}\}", runs_on):
        matrix = (job.get("strategy") or {}).get("matrix") or {}
        values += [str(v) for v in matrix.get(name, []) if not isinstance(v, dict)]
        values += [str(i[name]) for i in matrix.get("include", []) if name in i]
    return any("ubuntu" in v or "linux" in v.lower() for v in values)


def active_on_linux(step):
    """A step that a Linux runner runs: no condition on the OS, or one naming Linux."""
    condition = str(step.get("if", ""))
    return "runner.os" not in condition or "Linux" in condition


def check_job(label, job, errors):
    if not runs_on_linux(job):
        return False
    steps = [s for s in job.get("steps") or [] if isinstance(s, dict) and "run" in s]
    builds = [i for i, s in enumerate(steps) if active_on_linux(s) and step_builds_app(s["run"])]
    if not builds:
        return False
    first = builds[0]
    installs = [i for i, s in enumerate(steps) if runs_shared(s["run"])]
    if not installs:
        errors.append(
            f"[{label}] builds or publishes bekoedit on Linux (step "
            f"{steps[first].get('name', first)!r}) but never runs {SHARED}"
        )
    elif not any(i < first and active_on_linux(steps[i]) for i in installs):
        errors.append(
            f"[{label}] runs {SHARED} only after, or only off Linux from, its first "
            f"bekoedit build step ({steps[first].get('name', first)!r})"
        )
    return True


def main(argv):
    root = pathlib.Path(".")
    if argv[:1] == ["--root"]:
        if len(argv) != 2:
            raise SystemExit("--root needs exactly one directory")
        root = pathlib.Path(argv[1])
    errors = []
    checked = 0

    shared = root / SHARED
    if not shared.is_file():
        errors.append(f"{SHARED} is missing")
    else:
        text = shared.read_text()
        errors += [f"{SHARED} does not list {p}" for p in PACKAGES if p not in text]

    workflows = sorted((root / ".github").rglob("*.y*ml"))
    if not workflows:
        errors.append("no workflow files found under .github")
    for path in workflows:
        document = yaml.safe_load(path.read_text()) or {}
        for job_id, job in (document.get("jobs") or {}).items():
            label = f"{path.name} / {job_id}"
            before = len(errors)
            if check_job(label, job, errors):
                checked += 1
                if len(errors) == before:
                    print(f"ok   [{label}] runs {SHARED} before it builds bekoedit")
            for step in job.get("steps") or []:
                found = raw_install(str(step.get("run", ""))) if isinstance(step, dict) else []
                if found:
                    errors.append(
                        f"[{label}] step {step.get('name', '(unnamed)')!r} installs "
                        f"{', '.join(found)} with a raw apt install; use {SHARED}"
                    )

    for path in sorted((root / "scripts").glob("*")):
        if not path.is_file() or path.relative_to(root).as_posix() == SHARED:
            continue
        try:
            found = raw_install(path.read_text())
        except UnicodeDecodeError:
            continue
        if found:
            errors.append(
                f"[{path.relative_to(root).as_posix()}] installs {', '.join(found)} "
                f"with a raw apt install; use {SHARED}"
            )

    if not checked:
        errors.append("no job that builds bekoedit on Linux was found; the check would be vacuous")
    for error in errors:
        print(f"FAIL {error}")
    print(f"check-linux-build-deps: {checked} job(s) build bekoedit on Linux, {len(errors)} error(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
