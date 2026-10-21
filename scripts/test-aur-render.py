#!/usr/bin/env python3
"""Self-test for scripts/aur-render.py (RFC-045 slice 3b).

Pure checks of the renderer: each refusal, one full render of each real template
compared with an expected file, the source-order sums, and that nothing from the
template (markers, template-only comments, a SKIP) survives into the AUR copy.
The `.SRCINFO` step is tested with a stub `makepkg` on PATH, and additionally
with the real one when it is installed (it is on an Arch-family host, not on
GitHub's Ubuntu runner, where that case is skipped and says so).

Usage: python3 scripts/test-aur-render.py
Regenerate the expected files after a deliberate template change with:
       python3 scripts/test-aur-render.py --write-expected
"""

import hashlib
import importlib.util
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True  # importing aur-render.py must not leave a __pycache__ in scripts/
REPO = pathlib.Path(__file__).resolve().parent.parent
SCRIPTS = REPO / "scripts"
TESTDATA = SCRIPTS / "testdata" / "aur"
RENDER = SCRIPTS / "aur-render.py"
TEMPLATES = {
    "bekoedit": REPO / "packaging/linux/PKGBUILD",
    "bekoedit-bin": REPO / "packaging/linux/bin/PKGBUILD",
}
FAKE_VERSION = "9.9.9"

spec = importlib.util.spec_from_file_location("aur_render", RENDER)
aur_render = importlib.util.module_from_spec(spec)
spec.loader.exec_module(aur_render)

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


def fake_files(directory, count):
    files = []
    for index in range(count):
        path = directory / f"source-{index}"
        path.write_bytes(f"fixture file {index}\n".encode())
        files.append(path)
    return files


def versioned(template_text, version=FAKE_VERSION):
    return re.sub(r"^pkgver=.*$", f"pkgver={version}", template_text, count=1, flags=re.M)


def refuses(name, version, template, files, mention):
    try:
        aur_render.render(version, template, files)
    except aur_render.Refusal as refusal:
        check(name, mention in str(refusal), f"refused, but with: {refusal}")
    else:
        check(name, False, "was not refused")


def real_render(work, key):
    template = versioned(TEMPLATES[key].read_text())
    count = aur_render.array_entries(
        re.search(r"^sha256sums=\(([^)]*)\)", template, re.M).group(1)
    ).__len__()
    return aur_render.render(FAKE_VERSION, template, fake_files(work, count))


def expected_cases(work):
    for key in TEMPLATES:
        rendered = real_render(pathlib.Path(tempfile.mkdtemp(dir=work)), key)
        expected = (TESTDATA / f"{key}.PKGBUILD").read_text()
        check(f"a full render of the {key} template equals its expected file", rendered == expected,
              "(regenerate with --write-expected after a deliberate template change)")
        check(f"the {key} render keeps no marker, no SKIP and no template-only phrase",
              not re.search(r"template-only|>>>|<<<|\bSKIP\b|\bTEMPLATE\b|makepkg -si|check-packaging|aur-render|this machine",
                            rendered), rendered)
        sums = re.search(r"^sha256sums=\((.*)\)$", rendered, re.M).group(1)
        check(f"the {key} render has only real 64-hex sums",
              all(re.fullmatch(r"'[0-9a-f]{64}'", part) for part in sums.split()), sums)
        check(f"the {key} render keeps pkgrel=1 and the version",
              f"pkgver={FAKE_VERSION}\n" in rendered and "pkgrel=1\n" in rendered)


def refusal_cases(work):
    template = versioned(TEMPLATES["bekoedit-bin"].read_text())
    files = fake_files(pathlib.Path(tempfile.mkdtemp(dir=work)), 3)
    refuses("a template whose pkgver is not the version is refused", "9.9.8", template, files, "pkgver is 9.9.9, not 9.9.8")
    refuses("a template whose pkgrel is not 1 is refused", FAKE_VERSION,
            template.replace("pkgrel=1", "pkgrel=2"), files, "pkgrel is 2")
    refuses("fewer files than sha256sums is refused", FAKE_VERSION, template, files[:2], "2 files were given for 3")
    refuses("more files than sha256sums is refused", FAKE_VERSION, template, files + files[:1], "4 files were given for 3")
    refuses("a template with more sums than sources is refused", FAKE_VERSION,
            template.replace("sha256sums=('SKIP' 'SKIP' 'SKIP')", "sha256sums=('SKIP' 'SKIP' 'SKIP' 'SKIP')"),
            files + files[:1], "3 source entries but 4 sha256sums")
    refuses("a sum that is not SKIP in the template is refused", FAKE_VERSION,
            template.replace("sha256sums=('SKIP' 'SKIP' 'SKIP')", "sha256sums=('SKIP' 'SKIP' 'abc')"),
            files, "holds a real sum")
    refuses("a SKIP left outside the sums (an unmarked comment) is refused", FAKE_VERSION,
            template.replace("pkgrel=1", "pkgrel=1\n# SKIP is permanent here"), files, "a SKIP remains")
    refuses("a file that does not exist is refused", FAKE_VERSION, template,
            [files[0], files[1], files[2].with_name("missing")], "is not a file")
    refuses("a version that is not bare SemVer is refused", "v9.9.9", template, files, "not bare SemVer")
    refuses("an unclosed template-only block is refused", FAKE_VERSION,
            template + "\n# >>> template-only\n# never closed\n", files, "never closed")
    refuses("a closing marker without an opening one is refused", FAKE_VERSION,
            template + "\n# <<< template-only\n", files, "without an opening marker")
    refuses("a nested opening marker is refused", FAKE_VERSION,
            template + "\n# >>> template-only\n# >>> template-only\n# <<< template-only\n", files, "inside an open")


def order_cases(work):
    template = versioned(TEMPLATES["bekoedit-bin"].read_text())
    files = fake_files(pathlib.Path(tempfile.mkdtemp(dir=work)), 3)
    rendered = aur_render.render(FAKE_VERSION, template, files)
    sums = re.findall(r"[0-9a-f]{64}", re.search(r"^sha256sums=\((.*)\)$", rendered, re.M).group(1))
    check("the sums are those of the files, in the order given",
          sums == [hashlib.sha256(f.read_bytes()).hexdigest() for f in files])
    swapped = aur_render.render(FAKE_VERSION, template, [files[1], files[0], files[2]])
    check("giving the files in another order changes the sums", swapped != rendered)


def srcinfo_cases(work):
    template = TEMPLATES["bekoedit"].read_text()
    # A stub makepkg: records where it ran and what PKGBUILD it saw, prints a fixed .SRCINFO.
    for behaviour, code in (("works", 0), ("fails", 3)):
        stubs = pathlib.Path(tempfile.mkdtemp(dir=work))
        stub = stubs / "makepkg"
        stub.write_text(
            "#!/bin/sh\n"
            f'echo "$@" > "{stubs}/args"; pwd > "{stubs}/cwd"; test -f PKGBUILD && echo seen > "{stubs}/pkgbuild"\n'
            f'if [ {code} -eq 0 ]; then echo "pkgbase = stub"; else echo boom >&2; fi\nexit {code}\n'
        )
        stub.chmod(0o755)
        out = pathlib.Path(tempfile.mkdtemp(dir=work)) / "out"
        src = pathlib.Path(tempfile.mkdtemp(dir=work)) / "tarball"
        src.write_bytes(b"x")
        done = subprocess.run(
            [sys.executable, str(RENDER), FAKE_VERSION, str(_versioned_file(work, template)), str(out), str(src)],
            env={**os.environ, "PATH": f"{stubs}:{os.environ['PATH']}"}, capture_output=True, text=True)
        if code == 0:
            check("the .SRCINFO comes from `makepkg --printsrcinfo`, run in the output directory",
                  done.returncode == 0 and (stubs / "args").read_text().strip() == "--printsrcinfo"
                  and pathlib.Path((stubs / "cwd").read_text().strip()).resolve() == out.resolve()
                  and (stubs / "pkgbuild").exists() and (out / ".SRCINFO").read_text() == "pkgbase = stub\n",
                  done.stderr)
        else:
            check("a failing makepkg --printsrcinfo fails the render and leaves no .SRCINFO",
                  done.returncode == 1 and not (out / ".SRCINFO").exists() and "boom" in done.stderr, done.stderr)

    if shutil.which("makepkg") is None:
        print("skip real makepkg --printsrcinfo (makepkg is not installed on this machine)")
        return
    for key, count in (("bekoedit", 1), ("bekoedit-bin", 3)):
        out = pathlib.Path(tempfile.mkdtemp(dir=work)) / "out"
        files = fake_files(pathlib.Path(tempfile.mkdtemp(dir=work)), count)
        done = subprocess.run(
            [sys.executable, str(RENDER), FAKE_VERSION, str(_versioned_file(work, TEMPLATES[key].read_text())),
             str(out), *map(str, files)], capture_output=True, text=True)
        srcinfo = (out / ".SRCINFO").read_text() if (out / ".SRCINFO").exists() else ""
        check(f"real makepkg: {key}'s .SRCINFO names the version, the package and every sum",
              done.returncode == 0 and f"pkgver = {FAKE_VERSION}" in srcinfo and f"pkgname = {key}" in srcinfo
              and srcinfo.count("sha256sums = ") == count, done.stderr + srcinfo)


def _versioned_file(work, template_text):
    path = pathlib.Path(tempfile.mkdtemp(dir=work)) / "PKGBUILD"
    path.write_text(versioned(template_text))
    return path


def main():
    if "--write-expected" in sys.argv:
        TESTDATA.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory() as work:
            for key in TEMPLATES:
                (TESTDATA / f"{key}.PKGBUILD").write_text(
                    real_render(pathlib.Path(tempfile.mkdtemp(dir=work)), key))
        print(f"wrote expected files in {TESTDATA}")
        return 0
    with tempfile.TemporaryDirectory() as work:
        work = pathlib.Path(work)
        expected_cases(work)
        refusal_cases(work)
        order_cases(work)
        srcinfo_cases(work)
    print(f"test-aur-render: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
