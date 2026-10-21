#!/usr/bin/env python3
"""Structure tests for the AUR workflows and the pinned host key (RFC-045 slice 3b).

The AUR proof runs only in an Arch container and the push only at a real tag, so
what can be checked headlessly is checked here: that the key can only be reached
from the push job in release.yml, that job's shape, that the proof uploads the
hidden .SRCINFO, and that the pinned host keys are the ones documented.

Usage: python3 scripts/test-aur-workflows.py
"""

import base64
import hashlib
import pathlib
import re
import sys

import yaml

REPO = pathlib.Path(__file__).resolve().parent.parent
WORKFLOWS = {path.name: yaml.safe_load(path.read_text()) for path in sorted((REPO / ".github/workflows").glob("*.yml"))}
RAW = {path.name: path.read_text() for path in sorted((REPO / ".github/workflows").glob("*.yml"))}
RELEASE = WORKFLOWS["release.yml"]
AUR = WORKFLOWS["aur.yml"]
ARCH = WORKFLOWS["arch-build.yml"]

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


def triggers(workflow):
    return set(workflow.get(True, workflow.get("on")))


def steps_of(job):
    return job.get("steps", [])


def run_text(job):
    return "\n".join(s.get("run", "") for s in steps_of(job))


def aur_workflow_cases():
    check("aur.yml is started only by workflow_call and workflow_dispatch (the dry run)",
          triggers(AUR) == {"workflow_call", "workflow_dispatch"}, str(triggers(AUR)))
    check("aur.yml never mentions the key, the environment, or a push",
          not re.search(r"AUR_SSH_PRIVATE_KEY|environment:|secrets\.|aur-push\.sh|git push", RAW["aur.yml"]),
          "a dry run must hold no key and push nothing")
    prove = AUR["jobs"].get("prove", {})
    check("the proof runs in archlinux:base pinned by digest, the digest arch-build.yml uses",
          prove.get("container", {}).get("image") == ARCH["jobs"]["build-arch"]["container"]["image"]
          and re.fullmatch(r"archlinux:base@sha256:[0-9a-f]{64}", str(prove.get("container", {}).get("image"))) is not None,
          str(prove.get("container")))
    upload = [s for s in steps_of(prove) if s.get("uses", "").startswith("actions/upload-artifact@")]
    check("the proof uploads aur-out as 'aur-packages' with hidden files included (.SRCINFO is hidden)",
          len(upload) == 1 and upload[0].get("with", {}).get("name") == "aur-packages"
          and upload[0].get("with", {}).get("path") == "aur-out"
          and upload[0].get("with", {}).get("include-hidden-files") is True,
          str(upload))
    check("the proof runs scripts/aur-prove.sh, with AUR_MODE chosen by the ref type",
          "bash scripts/aur-prove.sh" in run_text(prove) and "AUR_MODE=tag" in run_text(prove)
          and "AUR_MODE=dry-run" in run_text(prove))
    check("the Arch archive is built for a dry run only, and not at a tag",
          AUR["jobs"].get("arch-archive", {}).get("if") == "github.ref_type != 'tag'"
          and AUR["jobs"].get("arch-archive", {}).get("uses") == "./.github/workflows/arch-build.yml",
          str(AUR["jobs"].get("arch-archive")))


def release_cases():
    jobs = RELEASE["jobs"]
    check("release.yml is started only by a tag push (no workflow_dispatch can reach aur-push)",
          triggers(RELEASE) == {"push"} and set(RELEASE[True]["push"]) == {"tags"}, str(triggers(RELEASE)))
    check("aur-prove needs publish, and calls the reusable workflow",
          jobs.get("aur-prove", {}).get("needs") == "publish"
          and jobs.get("aur-prove", {}).get("uses") == "./.github/workflows/aur.yml")
    push = jobs.get("aur-push", {})
    check("aur-push needs aur-prove and uses the aur environment",
          push.get("needs") == "aur-prove" and push.get("environment") == "aur",
          f"needs={push.get('needs')!r} environment={push.get('environment')!r}")
    check("publish-crates neither needs the AUR jobs nor is needed by them",
          jobs.get("publish-crates", {}).get("needs") == "publish"
          and all(jobs.get(name, {}).get("needs") not in ("publish-crates", ["publish-crates"])
                  for name in ("aur-prove", "aur-push")))
    uses = [s["uses"] for s in steps_of(push) if "uses" in s]
    check("aur-push uses only actions/checkout and actions/download-artifact, each pinned by commit",
          len(uses) == 2 and all(re.fullmatch(r"actions/(checkout|download-artifact)@[0-9a-f]{40}", u.split(" ")[0]) for u in uses),
          str(uses))
    check("aur-push runs no project build (no cargo, npm, rustup, make, pip)",
          not re.search(r"\b(cargo|npm|npx|rustup|make|pip|pip3)\b", run_text(push)), run_text(push))
    check("the only repository files aur-push runs are scripts/aur-push.sh and the host-key file",
          set(re.findall(r"\b(?:bash|sh|python3?)\s+(\S+)", run_text(push))) <= {"scripts/aur-push.sh"}
          and "packaging/linux/aur-known-hosts" in run_text(push), run_text(push))
    check("the key is a secret of this job's environment, referenced in exactly one place",
          RAW["release.yml"].count("secrets.AUR_SSH_PRIVATE_KEY") == 1
          and "secrets.AUR_SSH_PRIVATE_KEY" in str([s for s in steps_of(push) if s.get("name") == "Write the SSH key"]))
    check("no other workflow references the key", all("AUR_SSH_PRIVATE_KEY" not in text for name, text in RAW.items() if name != "release.yml"))
    write = "\n".join(s.get("run", "") for s in steps_of(push) if s.get("name") == "Write the SSH key")
    check("the key is written with umask 077 and chmod 600 under RUNNER_TEMP, and never echoed or put on a command line",
          "umask 077" in write and 'chmod 600 "$RUNNER_TEMP/aur_key"' in write
          and write.count("$AUR_SSH_PRIVATE_KEY") == 1 and re.search(r"printf '%s\\n' \"\$AUR_SSH_PRIVATE_KEY\" > ", write)
          and not re.search(r"\becho\b|\bset -x\b|\bcurl\b|\bbase64\b", write), write)
    remove = [s for s in steps_of(push) if s.get("name") == "Remove the SSH key"]
    check("the key is removed in a step that runs whatever happened (if: always())",
          len(remove) == 1 and remove[0].get("if") == "always()" and "rm -f" in remove[0].get("run", "")
          and steps_of(push).index(remove[0]) == len(steps_of(push)) - 1,
          str(remove))
    pushing = ([s for s in steps_of(push) if s.get("name") == "Push both packages"] or [{}])[0]
    ssh = pushing.get("run", "")
    check("ssh pins the host key file, checks it strictly, and offers only this identity",
          "StrictHostKeyChecking=yes" in ssh and "IdentitiesOnly=yes" in ssh
          and "UserKnownHostsFile=$GITHUB_WORKSPACE/packaging/linux/aur-known-hosts" in ssh and "-i $RUNNER_TEMP/aur_key" in ssh, ssh)
    check("the commit identity comes from the environment's variables, not hard-coded",
          pushing.get("env") == {"AUR_GIT_NAME": "${{ vars.AUR_GIT_NAME }}", "AUR_GIT_EMAIL": "${{ vars.AUR_GIT_EMAIL }}"},
          str(pushing.get("env")))
    check("both packages are attempted even if the first fails, and the job fails if either did",
          "for package in bekoedit bekoedit-bin" in ssh and "|| failed=1" in ssh and 'exit "$failed"' in ssh)
    check("it pushes aur-out/<package> to the AUR over ssh, with no force",
          'bash scripts/aur-push.sh "aur-out/$package" "ssh://aur@aur.archlinux.org/$package.git"' in ssh
          and "--force" not in ssh)
    check("no workflow other than release.yml has a job named aur-push or uses the aur environment",
          all(("aur-push" not in w.get("jobs", {})) and "environment: aur" not in RAW[n] for n, w in WORKFLOWS.items() if n != "release.yml"))


def host_key_cases():
    lines = (REPO / "packaging/linux/aur-known-hosts").read_text().splitlines()
    keys = [line.split() for line in lines if line.strip() and not line.startswith("#")]
    documented = dict(re.findall(r"^#\s+(Ed25519|ECDSA|RSA)\s+(SHA256:\S+)$", "\n".join(lines), re.M))
    check("the host-key file pins aur.archlinux.org's three key types", {k[1] for k in keys} == {"ssh-rsa", "ecdsa-sha2-nistp256", "ssh-ed25519"}
          and all(k[0] == "aur.archlinux.org" for k in keys), str(keys))
    names = {"ssh-rsa": "RSA", "ecdsa-sha2-nistp256": "ECDSA", "ssh-ed25519": "Ed25519"}
    for host, kind, blob in keys:
        fingerprint = "SHA256:" + base64.b64encode(hashlib.sha256(base64.b64decode(blob)).digest()).decode().rstrip("=")
        check(f"the pinned {names[kind]} key has the fingerprint documented in the file's header ({fingerprint})",
              documented.get(names[kind]) == fingerprint, str(documented))


def main():
    for group in (aur_workflow_cases, release_cases, host_key_cases):
        try:
            group()
        except Exception as error:  # a malformed workflow is a named failure, not a traceback
            check(f"{group.__name__} could read the workflows it checks", False, f"{type(error).__name__}: {error}")
    print(f"test-aur-workflows: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
