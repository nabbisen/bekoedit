#!/usr/bin/env python3
"""Self-test for .github/workflows/arch-build.yml and its wiring (RFC-045 slice 3a).

The Arch build itself only runs in CI, in an Arch container. What can be checked
headlessly is checked here, by running the workflow's own shell steps, extracted
from the YAML, against fixtures: the libxdo assertion, the version resolution and
the packaging, plus the structure around them (triggers, the pinned image, and
release.yml's use of the job). Nothing is installed and no container is started.

Usage: python3 scripts/test-arch-build-workflow.py
"""

import hashlib
import os
import pathlib
import re
import subprocess
import sys
import tarfile
import tempfile

import yaml

REPO = pathlib.Path(__file__).resolve().parent.parent
ARCH = yaml.safe_load((REPO / ".github/workflows/arch-build.yml").read_text())
RELEASE = yaml.safe_load((REPO / ".github/workflows/release.yml").read_text())
JOB = ARCH["jobs"]["build-arch"]
TARGET = "x86_64-unknown-linux-gnu"

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


class MissingStep(Exception):
    pass


def step_run(name):
    found = [s["run"] for s in JOB["steps"] if s.get("name") == name]
    if len(found) != 1:
        raise MissingStep(f"expected exactly one step named {name!r}, found {len(found)}")
    return found[0]


def run_step(name, cwd, env, path_prefix=None):
    environ = {**os.environ, "TARGET": TARGET, **env}
    if path_prefix:
        environ["PATH"] = f"{path_prefix}:{environ['PATH']}"
    return subprocess.run(
        ["bash", "-c", step_run(name)], cwd=cwd, env=environ, capture_output=True, text=True
    )


def stub_readelf(directory, needed):
    stub = directory / "readelf"
    lines = "\n".join(
        f" 0x0000000000000001 (NEEDED)             Shared library: [{lib}]" for lib in needed
    )
    stub.write_text(f"#!/bin/sh\ncat <<'EOF'\n{lines}\nEOF\n")
    stub.chmod(0o755)


def libxdo_cases(work):
    name = "The binary links libxdo.so.4 and not libxdo.so.3"
    cwd = pathlib.Path(tempfile.mkdtemp(dir=work))
    (cwd / "target" / TARGET / "release").mkdir(parents=True)
    (cwd / "target" / TARGET / "release" / "bekoedit").write_bytes(b"fixture")
    expectations = [
        (["libxdo.so.4", "libc.so.6"], True, ""),
        (["libxdo.so.3", "libc.so.6"], False, "does not list libxdo.so.4"),
        (["libxdo.so.4", "libxdo.so.3"], False, "lists libxdo.so.3"),
        (["libc.so.6"], False, "does not list libxdo.so.4"),
    ]
    for needed, passes, message in expectations:
        stubs = pathlib.Path(tempfile.mkdtemp(dir=work))
        stub_readelf(stubs, needed)
        done = run_step(name, cwd, {}, str(stubs))
        ok = (done.returncode == 0) if passes else (done.returncode != 0 and message in done.stderr)
        check(f"libxdo assertion with NEEDED {needed}: {'passes' if passes else 'fails'}", ok,
              f"exit {done.returncode}: {done.stderr.strip()}")
    # The failure quotes the NEEDED lines it saw.
    stubs = pathlib.Path(tempfile.mkdtemp(dir=work))
    stub_readelf(stubs, ["libxdo.so.3"])
    done = run_step(name, cwd, {}, str(stubs))
    check("a failing libxdo assertion quotes the NEEDED lines", "Shared library: [libxdo.so.3]" in done.stdout,
          done.stdout)


def version_cases(work):
    name = "Resolve the version"

    def resolve(ref_type, ref_name, cargo_version="0.18.1"):
        cwd = pathlib.Path(tempfile.mkdtemp(dir=work))
        (cwd / "Cargo.toml").write_text(
            f'[workspace]\nmembers = []\n\n[workspace.package]\nversion = "{cargo_version}"\n'
            'edition = "2024"\n\n[workspace.dependencies]\nfoo = { version = "9.9.9" }\n'
        )
        env_file = cwd / "github_env"
        env_file.write_text("")
        done = run_step(
            name, cwd,
            {"GITHUB_REF_TYPE": ref_type, "GITHUB_REF_NAME": ref_name, "GITHUB_ENV": str(env_file)},
        )
        return done, env_file.read_text()

    done, env = resolve("tag", "1.2.3")
    check("a tag run takes the tag as the version", done.returncode == 0 and "VERSION=1.2.3" in env, done.stderr)
    done, env = resolve("tag", "v1.2")
    check("a tag that is not bare SemVer fails", done.returncode != 0 and "invalid bare SemVer" in done.stderr, done.stderr)
    done, env = resolve("branch", "main", "0.18.1")
    check("a manual run takes the workspace version, not a dependency's",
          done.returncode == 0 and "VERSION=0.18.1" in env, f"{done.stderr} {env}")


def package_cases(work):
    cwd = pathlib.Path(tempfile.mkdtemp(dir=work))
    (cwd / "target" / TARGET / "release").mkdir(parents=True)
    (cwd / "target" / TARGET / "release" / "bekoedit").write_bytes(b"arch-binary")
    for document in ("README.md", "LICENSE", "NOTICE", "CHANGELOG.md"):
        (cwd / document).write_text(document)
    (cwd / "scripts").mkdir()
    (cwd / "scripts" / "run-linux.sh").write_text("#!/bin/sh\n")
    env_file = cwd / "github_env"
    env_file.write_text("")
    done = run_step("Package", cwd, {"VERSION": "1.2.3", "GITHUB_ENV": str(env_file)})
    name = f"bekoedit-1.2.3-{TARGET}-arch.tar.gz"
    archive = cwd / name
    check("packaging succeeds and names the archive for the Arch target", done.returncode == 0 and archive.is_file(),
          done.stderr)
    if not archive.is_file():
        return
    with tarfile.open(archive) as opened:
        members = sorted(m.name for m in opened.getmembers())
        modes = {m.name: m.mode for m in opened.getmembers()}
    check("the archive has exactly the six members of the Ubuntu archive",
          members == sorted(["bekoedit", "README.md", "LICENSE", "NOTICE", "CHANGELOG.md", "run-linux.sh"]),
          str(members))
    check("run-linux.sh is executable in the archive", modes.get("run-linux.sh", 0) & 0o111 != 0)
    record = (cwd / f"{name}.sha256").read_text()
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    check("the .sha256 is one lowercase record in the same format as the other targets",
          record == f"{digest}  {name}\n", record)
    check("the step exports ARCHIVE for the upload step", f"ARCHIVE={name}" in env_file.read_text())


def structure_cases():
    triggers = ARCH.get(True, ARCH.get("on"))
    check("the Arch build is triggered only by workflow_call and workflow_dispatch",
          set(triggers) == {"workflow_call", "workflow_dispatch"}, str(triggers))
    image = JOB["container"]["image"]
    check("the image is archlinux:base pinned by digest", re.fullmatch(r"archlinux:base@sha256:[0-9a-f]{64}", image) is not None, image)
    harness = (REPO / "scripts/check-linux-portability-containers.sh").read_text()
    check("it is the same digest as slice 2's portability harness pins", image in harness)
    check("the toolchain step installs rustc 1.88.0 and checks the host",
          "rustup toolchain install 1.88.0" in step_run("Install the pinned Rust toolchain")
          and 'test "$RUSTC_HOST" = "$TARGET"' in step_run("Install the pinned Rust toolchain"))
    runs = "\n".join(s.get("run", "") for s in JOB["steps"])
    for needle in (
        "cargo +1.88.0 metadata --locked",
        'cargo +1.88.0 test --locked --target "$TARGET"',
        'cargo +1.88.0 build -p bekoedit --release --locked --target "$TARGET"',
        "git diff --exit-code -- Cargo.lock",
        "--headless-smoke",
    ):
        check(f"the job runs {needle!r}", needle in runs)
    check("the headless smoke runs with no display, as ci.yml does",
          "env -u DISPLAY -u WAYLAND_DISPLAY timeout 10" in step_run("Start the binary once, headless"))
    upload = [s for s in JOB["steps"] if s.get("name") == "Upload the Arch archive"][0]
    check("the artifact name starts with release- so publish's pattern picks it up",
          upload["with"]["name"].startswith("release-"), upload["with"]["name"])

    jobs = RELEASE["jobs"]
    check("release.yml calls the reusable Arch workflow", jobs["build-arch"].get("uses") == "./.github/workflows/arch-build.yml")
    check("publish needs both build and build-arch", set(jobs["publish"]["needs"]) == {"build", "build-arch"},
          str(jobs["publish"]["needs"]))
    publish_step = [s for s in jobs["publish"]["steps"] if s.get("uses", "").startswith("softprops/")][0]
    files = [f.strip() for f in publish_step["with"]["files"].splitlines() if f.strip()]
    check("the release gets eight files, the two Arch ones among them", len(files) == 8 and
          sum("-arch.tar.gz" in f for f in files) == 2, str(files))
    portability = [s for s in jobs["publish"]["steps"] if "Arch archive portability" in s.get("name", "")]
    check("the publish job runs the Arch-only, no-exemption portability check on the Arch archive",
          len(portability) == 1 and "--only arch --no-exemptions" in portability[0]["run"]
          and "-arch.tar.gz" in portability[0]["run"])
    check("publish-crates is still just after publish, and unchanged in its needs",
          jobs["publish-crates"]["needs"] == "publish")


def step_index(name):
    return [i for i, s in enumerate(JOB["steps"]) if s.get("name") == name]


TRUST = "Trust the workspace for later git commands"
LOCKFILE = "Confirm committed lockfile is unchanged"


def trust_structure_cases():
    """Task 068: the later git commands trust the workspace, and only it."""
    trust, lockfile = step_index(TRUST), step_index(LOCKFILE)
    checkouts = [i for i, s in enumerate(JOB["steps"]) if s.get("uses", "").startswith("actions/checkout@")]
    check("the trust step exists exactly once", len(trust) == 1, str(trust))
    if len(trust) != 1 or len(lockfile) != 1:
        return
    run = JOB["steps"][trust[0]]["run"]
    check("the trust step adds $GITHUB_WORKSPACE to the global safe.directory",
          'git config --global --add safe.directory "$GITHUB_WORKSPACE"' in run, run)
    check("the trust step never trusts '*'", "*" not in run, run)
    check("the trust step comes after the last checkout", trust[0] > max(checkouts), f"{trust} vs {checkouts}")
    check("the trust step comes before the lockfile step", trust[0] < lockfile[0], f"{trust} vs {lockfile}")
    check("the lockfile step is exactly the plain git diff, not weakened",
          JOB["steps"][lockfile[0]]["run"].strip() == "git diff --exit-code -- Cargo.lock"
          and "continue-on-error" not in JOB["steps"][lockfile[0]], str(JOB["steps"][lockfile[0]]))


def trust_behaviour_cases(work):
    """Task 068, headless: git's own switch for a work tree owned by someone else."""
    def scratch_repo():
        repo = pathlib.Path(tempfile.mkdtemp(dir=work))
        env = {**os.environ, "GIT_CONFIG_GLOBAL": str(repo.parent / f"{repo.name}.gitconfig"),
               "GIT_CONFIG_SYSTEM": "/dev/null"}
        env.pop("GIT_TEST_ASSUME_DIFFERENT_OWNER", None)
        (repo / "Cargo.lock").write_text("lock\n")
        for command in (["git", "init", "-q"], ["git", "add", "Cargo.lock"],
                        ["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "lock"]):
            subprocess.run(command, cwd=repo, env=env, check=True, capture_output=True)
        return repo, {**env, "GIT_TEST_ASSUME_DIFFERENT_OWNER": "1", "GITHUB_WORKSPACE": str(repo)}

    def run_steps(repo, env, names):
        last = None
        for name in names:
            last = subprocess.run(["bash", "-c", step_run(name)], cwd=repo, env=env,
                                  capture_output=True, text=True)
        return last

    repo, env = scratch_repo()
    done = run_steps(repo, env, [LOCKFILE])
    check("without the trust step the lockfile step fails with 'Not a git repository'",
          done.returncode != 0 and "Not a git repository" in done.stderr,
          f"exit {done.returncode}: {done.stderr[:200]}")

    repo, env = scratch_repo()
    done = run_steps(repo, env, [TRUST, LOCKFILE])
    check("with the trust step the lockfile step passes on a clean Cargo.lock", done.returncode == 0,
          f"exit {done.returncode}: {done.stderr[:200]}")

    repo, env = scratch_repo()
    (repo / "Cargo.lock").write_text("changed\n")
    done = run_steps(repo, env, [TRUST, LOCKFILE])
    check("with the trust step the lockfile step still fails on a changed Cargo.lock",
          done.returncode == 1 and "Cargo.lock" in done.stdout, f"exit {done.returncode}: {done.stdout[:200]}")

    repo, env = scratch_repo()
    other = pathlib.Path(tempfile.mkdtemp(dir=work))
    done = run_steps(repo, {**env, "GITHUB_WORKSPACE": str(other)}, [TRUST, LOCKFILE])
    check("trusting a different directory does not make this repository trusted",
          done.returncode != 0 and "Not a git repository" in done.stderr, f"exit {done.returncode}")


def main():
    with tempfile.TemporaryDirectory() as work:
        work = pathlib.Path(work)
        for group in (structure_cases, trust_structure_cases, libxdo_cases, version_cases,
                      package_cases, trust_behaviour_cases):
            try:
                group() if group in (structure_cases, trust_structure_cases) else group(work)
            except MissingStep as error:
                check(f"{group.__name__}: every step it runs exists", False, str(error))
    print(f"test-arch-build-workflow: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
