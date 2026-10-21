#!/usr/bin/env python3
"""Self-test for scripts/aur-namcap-errors.py (RFC-045 slice 3b, review round 2).

The one excused namcap error is the debug package's build-id symlink; everything
else must still fail, the debug package's other errors included.

Usage: python3 scripts/test-aur-namcap-errors.py
"""

import importlib.util
import pathlib
import subprocess
import sys

sys.dont_write_bytecode = True
REPO = pathlib.Path(__file__).resolve().parent.parent
SCRIPT = REPO / "scripts" / "aur-namcap-errors.py"
spec = importlib.util.spec_from_file_location("aur_namcap_errors", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

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


# What namcap really printed for a debug package built here (task review round 2).
REAL_DEBUG = (
    "bekoedit-bin-debug W: Directory (usr/src/debug/bekoedit-bin) is empty\n"
    "bekoedit-bin-debug E: Symlink (usr/lib/debug/.build-id/ef/9deee3e5388dfd61be178dc3631d9ee28ce369) "
    "points to non-existing ../../../../bin/bekoedit\n"
)


def main():
    check("the real debug-package namcap output has no failing error", module.errors(REAL_DEBUG) == [])
    check("a warning never fails", module.errors("bekoedit W: Missing Maintainer tag\n") == [])
    check("an ordinary error fails", module.errors("bekoedit E: Dependency included and not needed (foo)\n")
          == ["bekoedit E: Dependency included and not needed (foo)"])
    check("the same symlink error in a package that is not -debug still fails",
          len(module.errors(REAL_DEBUG.replace("bekoedit-bin-debug", "bekoedit-bin"))) == 1)
    check("another error in the debug package still fails",
          len(module.errors(REAL_DEBUG + "bekoedit-bin-debug E: ELF file ('usr/lib/debug/x') outside of a valid path\n")) == 1)
    check("a symlink error outside usr/lib/debug/.build-id still fails",
          len(module.errors("bekoedit-bin-debug E: Symlink (usr/share/x) points to non-existing ../../../../bin/bekoedit\n")) == 1)
    check("a build-id symlink pointing somewhere other than up out of usr/ still fails",
          len(module.errors("bekoedit-bin-debug E: Symlink (usr/lib/debug/.build-id/ef/9dee) points to non-existing /etc/passwd\n")) == 1)
    check("a build-id symlink with an unexpected id shape still fails",
          len(module.errors("bekoedit-bin-debug E: Symlink (usr/lib/debug/.build-id/EF/ZZ) points to non-existing ../../../../bin/x\n")) == 1)
    done = subprocess.run([sys.executable, "-I", str(SCRIPT)], input=REAL_DEBUG, capture_output=True, text=True)
    check("the command line exits 0 and prints nothing for the real debug output", done.returncode == 0 and done.stdout == "")
    done = subprocess.run([sys.executable, "-I", str(SCRIPT)], input=REAL_DEBUG + "bekoedit E: boom\n", capture_output=True, text=True)
    check("the command line prints the failing line and exits 1", done.returncode == 1 and done.stdout == "bekoedit E: boom\n", done.stdout)

    prove = (REPO / "scripts/aur-prove.sh").read_text()
    code = "\n".join(line for line in prove.splitlines() if not line.lstrip().startswith("#"))
    check("aur-prove.sh fails the job on exactly the lines this script prints",
          'python "$REPO_DIR/scripts/aur-namcap-errors.py" < "$namcap_log"' in code and "grep -E ' E: '" not in code)
    print(f"test-aur-namcap-errors: {cases} case(s), {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
