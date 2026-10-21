#!/usr/bin/env python3
"""Self-test for scripts/aur-pick-package.py (RFC-045 slice 3b, review round 2).

The selection is a function of file names alone, so it is tested headlessly: the
proof must install exactly the package under proof and never its `-debug`
sibling, which Arch's default makepkg.conf builds.

Usage: python3 scripts/test-aur-pick-package.py
"""

import importlib.util
import pathlib
import subprocess
import sys

sys.dont_write_bytecode = True
REPO = pathlib.Path(__file__).resolve().parent.parent
PICK = REPO / "scripts" / "aur-pick-package.py"
spec = importlib.util.spec_from_file_location("aur_pick_package", PICK)
picker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(picker)

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


def f(package, version="0.18.1", release="1", arch="x86_64", ext="zst"):
    return f"/home/builder/build/x/{package}-{version}-{release}-{arch}.pkg.tar.{ext}"


def picks(name, files):
    try:
        return picker.pick(name, files)
    except ValueError as refusal:
        return refusal


def main():
    check("a package and its -debug sibling: only the package is picked",
          picks("bekoedit-bin", [f("bekoedit-bin"), f("bekoedit-bin-debug")]) == f("bekoedit-bin"))
    check("the order of the list does not matter",
          picks("bekoedit-bin", [f("bekoedit-bin-debug"), f("bekoedit-bin")]) == f("bekoedit-bin"))
    check("the package alone is picked", picks("bekoedit", [f("bekoedit")]) == f("bekoedit"))
    refusal = picks("bekoedit", [f("bekoedit-debug")])
    check("only the -debug package is refused, naming what was built",
          isinstance(refusal, ValueError) and "no built file is package 'bekoedit'" in str(refusal)
          and "bekoedit-debug-0.18.1-1-x86_64.pkg.tar.zst" in str(refusal), str(refusal))
    refusal = picks("bekoedit", [f("bekoedit"), f("bekoedit", release="2")])
    check("two candidates are refused, naming what was built",
          isinstance(refusal, ValueError) and "2 built files are package 'bekoedit'" in str(refusal), str(refusal))
    check("bekoedit does not match bekoedit-bin (a prefix is not the name)",
          isinstance(picks("bekoedit", [f("bekoedit-bin")]), ValueError))
    check("bekoedit-bin does not match bekoedit",
          isinstance(picks("bekoedit-bin", [f("bekoedit")]), ValueError))
    check("among all four built files, each name picks its own package",
          picks("bekoedit", [f("bekoedit-bin"), f("bekoedit-bin-debug"), f("bekoedit"), f("bekoedit-debug")]) == f("bekoedit")
          and picks("bekoedit-bin", [f("bekoedit"), f("bekoedit-debug"), f("bekoedit-bin-debug"), f("bekoedit-bin")]) == f("bekoedit-bin"))
    check("nothing built is refused", isinstance(picks("bekoedit", []), ValueError)
          and "built: nothing" in str(picks("bekoedit", [])))
    check("other compressions are read too (.xz, .gz and a bare .pkg.tar)",
          picks("bekoedit", [f("bekoedit", ext="xz")]) == f("bekoedit", ext="xz")
          and picks("bekoedit", [f("bekoedit", ext="gz")]) == f("bekoedit", ext="gz")
          and picks("bekoedit", ["/x/bekoedit-0.18.1-1-x86_64.pkg.tar"]) == "/x/bekoedit-0.18.1-1-x86_64.pkg.tar")
    check("a signature file is not a package", isinstance(picks("bekoedit", [f("bekoedit") + ".sig"]), ValueError))
    check("a version with dots and a name with dashes are split from the right",
          picks("a-b-c", [f("a-b-c", version="10.2.30", release="12")]) == f("a-b-c", version="10.2.30", release="12"))
    check("the arch field is not part of the name (any, x86_64)",
          picks("bekoedit-bin", [f("bekoedit-bin", arch="any")]) == f("bekoedit-bin", arch="any"))

    done = subprocess.run([sys.executable, "-I", str(PICK), "bekoedit-bin", f("bekoedit-bin-debug"), f("bekoedit-bin")],
                          capture_output=True, text=True)
    check("the command line prints the one file and exits 0",
          done.returncode == 0 and done.stdout.strip() == f("bekoedit-bin"), done.stderr)
    done = subprocess.run([sys.executable, "-I", str(PICK), "bekoedit", f("bekoedit-debug")], capture_output=True, text=True)
    check("the command line refuses with exit 1, and prints nothing on stdout",
          done.returncode == 1 and done.stdout == "" and "refused" in done.stderr, done.stderr)

    prove = (REPO / "scripts/aur-prove.sh").read_text()
    code = "\n".join(line for line in prove.splitlines() if not line.lstrip().startswith("#"))
    installs = [line.strip() for line in code.splitlines() if "pacman -U" in line]
    check("aur-prove.sh has one pacman -U, of the file the picker chose (never every built file)",
          installs == ['pacman -U --noconfirm "$package_file"']
          and 'package_file="$(python "$REPO_DIR/scripts/aur-pick-package.py" "$name" "${built_files[@]}")"' in code,
          str(installs))
    check("aur-prove.sh still runs namcap on every built file, the debug package included",
          'for built in ./*.pkg.tar.*; do namcap "$built"; done' in code)
    print(f"test-aur-pick-package: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
