#!/usr/bin/env python3
"""AGENT-01L read-only, non-authorizing native Linux cgroup-v2 diagnostic.

The original first-party custodian checks a protected, unwritable parent
cgroup.procs, while its non-root pre_exec child writes a new leaf's procs.
cgroup-v2 migration requires write permission at the common ancestor,
which is precisely that protected parent. This helper exposes the conflict;
it NEVER creates a group, migrates or kills a process, registers a runner,
changes privileges, or emits the positive AGENT01L acceptance marker.
"""
import errno
import json
import os
import pathlib
import platform
import stat
import sys

NOT_AVAILABLE = 78
SOURCE_CONFLICT = 79
CGROUP = pathlib.Path("/sys/fs/cgroup")


def emit(result, reason, **details):
    report = {
        "schema": "lu-agent01l-native-readonly-preflight-v1",
        "result": result,
        "reason": reason,
        "positive_cgroup_custody_accepted": False,
        "changed_kernel_state": False,
        **details,
    }
    print(json.dumps(report, sort_keys=True))
    return SOURCE_CONFLICT if result == "SOURCE_REQUIRES_ROOT_MEDIATED_ADMISSION" else NOT_AVAILABLE


def cgroup_mount():
    rows = []
    for line in pathlib.Path("/proc/self/mountinfo").read_text().splitlines():
        left, delim, right = line.partition(" - ")
        if not delim:
            continue
        fields = left.split()
        kind = right.split()
        if len(fields) >= 6 and len(kind) >= 1 and fields[4] == str(CGROUP):
            rows.append((kind[0], fields[5].split(",")))
    return rows


def own_cgroup():
    rows = pathlib.Path("/proc/self/cgroup").read_text().splitlines()
    if len(rows) != 1 or not rows[0].startswith("0::/"):
        raise ValueError("not exactly one unified cgroup-v2 process identity")
    relative = rows[0][4:]
    parts = pathlib.PurePosixPath(relative).parts
    if any(p in ("..", ".") for p in parts):
        raise ValueError("untrusted process cgroup path")
    root = CGROUP.joinpath(*parts)
    # A preflight must not follow a symlink outside the kernel mount.
    if not root.resolve(strict=True).is_relative_to(CGROUP.resolve(strict=True)):
        raise ValueError("process cgroup outside kernel mount")
    return root


def can_open_for_write_without_writing(path):
    flags = os.O_WRONLY | os.O_CLOEXEC | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(str(path), flags)
    except OSError as ex:
        if ex.errno in (errno.EACCES, errno.EPERM, errno.EROFS):
            return False, ex.errno
        raise
    else:
        os.close(fd)
        return True, 0


def main():
    if platform.system() != "Linux":
        return emit("NO_NATIVE_LINUX", "requires a real Linux kernel")
    uid = os.geteuid()
    if uid == 0:
        return emit("RUNNER_MUST_BE_NONROOT", "the original first-party fixture denies root")
    try:
        mounts = cgroup_mount()
        if len(mounts) != 1 or mounts[0][0] != "cgroup2":
            return emit("NO_AUTHENTIC_UNIFIED_MOUNT", "exact /sys/fs/cgroup cgroup2 missing")
        if "rw" not in mounts[0][1]:
            return emit("READ_ONLY_CGROUP_MOUNT", "kernel cgroup2 mount is not writable")
        root = own_cgroup()
        parent = root / "cgroup.procs"
        meta = parent.lstat()
        if not stat.S_ISREG(meta.st_mode) or parent.is_symlink():
            return emit("UNTRUSTED_CGROUP_PARENT", "parent cgroup.procs not a kernel file")
        writable, err = can_open_for_write_without_writing(parent)
        details = {
            "kernel_process_cgroup": str(root),
            "runner_euid": uid,
            "parent_procs_uid": meta.st_uid,
            "parent_procs_write_open_succeeded": writable,
            "parent_procs_open_errno": err,
            "parent_dir_creatable_by_uid": os.access(root, os.W_OK | os.X_OK),
            "kernel_parent_events_exists": (root / "cgroup.events").is_file(),
            "kernel_parent_controllers_exists": (root / "cgroup.controllers").is_file(),
        }
        if meta.st_uid == uid or writable:
            return emit("UNSAFE_WRITABLE_ANCESTOR",
                        "same-UID process can migrate to the parent; no custody guarantee",
                        **details)
        if not details["parent_dir_creatable_by_uid"]:
            return emit("NO_DELEGATED_CHILD_DIRECTORY",
                        "runner cannot create the required child cgroup", **details)
        if not details["kernel_parent_events_exists"]:
            return emit("MISSING_KERNEL_EVENTS",
                        "missing original cgroup.events evidence", **details)
        return emit("SOURCE_REQUIRES_ROOT_MEDIATED_ADMISSION",
                    "parent is protected, but original non-root pre_exec child "
                    "cannot migrate from parent into its new leaf: kernel requires "
                    "write permission on common-ancestor cgroup.procs",
                    **details)
    except (OSError, ValueError) as ex:
        return emit("KERNEL_EVIDENCE_UNAVAILABLE",
                    type(ex).__name__ + ": " + str(ex)[:250])


if __name__ == "__main__":
    sys.exit(main())
