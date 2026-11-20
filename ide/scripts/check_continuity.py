#!/usr/bin/env python3
"""PCP-01: Require continuity records on IDE implementation PRs.

Runs with only Python's standard library. It checks PR base vs head,
not the last commit, so multi-commit PRs cannot evade the gate.
"""
import subprocess
import sys

def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()

def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: check_continuity.py BASE_SHA HEAD_SHA")
    base, head = sys.argv[1:]
    changed = set(git("diff", "--name-only", "--diff-filter=ACMRT",
                      base, head, "--").splitlines())
    implementation = {p for p in changed if (
        p.startswith(("ide/app/", "ide/host/", "ide/proto/", "ide/sandbox/"))
        or p in {".github/workflows/ide-standalone.yml",
                 ".github/workflows/ide-alpha-windows.yml",
                 ".github/workflows/ide-sandbox-linux.yml"}
    )}
    if not implementation:
        print("PCP-01: no IDE implementation changed; no ledger gate required")
        return
    required = {"ide/CHANGELOG.md", "ide/PROJECT_STATE.md"}
    missing = required - changed
    if missing:
        raise SystemExit("PCP-01 FAIL: IDE implementation changed, but these "
                         "continuity documents were not updated: " + ", ".join(sorted(missing)))
    for path in sorted(required):
        diff = git("diff", "--unified=0", base, head, "--", path)
        added = [l[1:].strip() for l in diff.splitlines()
                 if l.startswith("+") and not l.startswith("+++")]
        substantive = [l for l in added
                       if l and not l.startswith(("**Updated:**", "**Last inspected", "# "))]
        if not substantive:
            raise SystemExit(f"PCP-01 FAIL: {path} has no substantive added entry")
    print("PCP-01 PASS: IDE implementation has changelog and current-state updates")

if __name__ == "__main__":
    main()
