#!/usr/bin/env python3
"""Host-only preflight for ONE ephemeral Lu C4 GitHub runner.

Run as the intended non-root runner service user, inside its final systemd
Delegate=yes service. This script does not register a runner, gain privileges,
reconfigure cgroups, mint evidence or accept a Lu release. It refuses unsafe
hosts and gives an exact diagnostic for missing OS capabilities.

Note: systemd's normal Delegate=yes often makes the service leaf cgroup.procs
writable by the delegated user. Lu's original same-UID cgroup guard instead
requires that file to be independently owned and not writable by that user.
A pass here must never be manufactured by faking permissions or remounting.
"""
import argparse
import json
import os
import pathlib
import secrets
import sys

CGROOT = pathlib.Path("/sys/fs/cgroup")
EXPECTED = "2fb253ab35bc3b451b9c07b89f5549aec26c32e3"

def require(ok, code):
    if not ok:
        raise RuntimeError(code)

def preflight():
    require(sys.platform == "linux", "LINUX_ONLY")
    require(os.geteuid() != 0, "ROOT_EXECUTION_DENIED")
    rows = pathlib.Path("/proc/self/cgroup").read_text(encoding="ascii").splitlines()
    own = [s[3:] for s in rows if s.startswith("0::")]
    require(len(rows) == len(own) == 1 and own[0].startswith("/") and own[0] != "/",
            "NO_DELEGATED_NONROOT_V2_CGROUP")
    root = CGROOT.resolve(strict=True)
    leaf = (root / own[0].lstrip("/")).resolve(strict=True)
    require(leaf.is_relative_to(root) and leaf != root and leaf.is_dir(),
            "INVALID_CGROUP_LEAF")
    mounts = pathlib.Path("/proc/self/mountinfo").read_text(encoding="ascii").splitlines()
    matches = [m for m in mounts
               if " - cgroup2 " in m and m.split(" - ", 1)[0].split()[4] == "/sys/fs/cgroup"
               and "rw" in m.split(" - ", 1)[0].split()[5].split(",")]
    require(len(matches) == 1, "CGROUP_V2_MOUNT_NOT_RW")
    require(os.getxattr(leaf, "user.delegate") == b"1", "SYSTEMD_DELEGATE_YES_NOT_PROVEN")
    require((leaf / "cgroup.controllers").is_file(), "NO_REAL_CGROUP_CONTROLLERS")
    original_procs = leaf / "cgroup.procs"
    require(original_procs.is_file(), "NO_REAL_CGROUP_PROCS")
    require(original_procs.stat().st_uid != os.geteuid(),
            "LU_UNSAFE_SAME_UID_ANCESTOR_PROCS")
    # O_WRONLY only. Never truncate or write a PID into the parent.
    try:
        fd = os.open(original_procs, os.O_WRONLY | os.O_CLOEXEC)
    except PermissionError:
        pass
    else:
        os.close(fd)
        raise RuntimeError("LU_UNSAFE_WRITABLE_ANCESTOR_PROCS")

    temporary = leaf / ("lu-c4-probe-" + secrets.token_hex(12))
    temporary.mkdir()
    try:
        require((temporary / "cgroup.kill").is_file(), "NO_REAL_CGROUP_KILL")
        require((temporary / "cgroup.procs").is_file(), "NO_CHILD_CGROUP_PROCS")
        require("populated 0" in (temporary / "cgroup.events").read_text().splitlines(),
                "CGROUP_NOT_RECURSIVELY_EMPTY")
        # This only checks the child's cgroup.procs write permission; the
        # unchanged original kernel test must still prove live migration,
        # setsid containment, host SIGKILL, receipt and restart independently.
        try:
            fd = os.open(temporary / "cgroup.procs", os.O_WRONLY | os.O_CLOEXEC)
        except PermissionError as error:
            raise RuntimeError("CHILD_CGROUP_PROCS_NOT_WRITABLE") from error
        else:
            os.close(fd)
    finally:
        temporary.rmdir()
    return {
        "status": "DELEGATION_PREFLIGHT_ONLY",
        "sourceSha": EXPECTED,
        "uid": os.geteuid(),
        "kernel": os.uname().release,
        "systemdDelegate": True,
        "protectedAncestorCgroupProcs": True,
        "childKernelKillAvailable": True,
        "originalKernelExecutionAccepted": False,
        "runnerRegistered": False,
        "releaseAuthorized": False,
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default=None,
                        help="optional path to a fresh JSON diagnostic file")
    args = parser.parse_args()
    try:
        result = preflight()
        code = 0
    except (RuntimeError, OSError, ValueError) as error:
        result = {"status": "BLOCKED", "reason": str(error),
                  "sourceSha": EXPECTED, "originalKernelExecutionAccepted": False,
                  "runnerRegistered": False, "releaseAuthorized": False}
        code = 78
    body = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        dest = pathlib.Path(args.output)
        # Never overwrite evidence from a previous attempt.
        with dest.open("x", encoding="utf-8") as stream:
            stream.write(body)
    print(body, end="")
    return code

if __name__ == "__main__":
    sys.exit(main())
