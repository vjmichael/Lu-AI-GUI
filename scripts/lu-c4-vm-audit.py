#!/usr/bin/env python3
"""Read-only Lu C4 disposable VM audit. NOT a custody or runner receipt."""
import json
import os
import pathlib
import pwd
import shutil
import stat
import subprocess
import sys

SHA = "2fb253ab35bc3b451b9c07b89f5549aec26c32e3"
ROOT = pathlib.Path("/srv/lu-c4")
ROLE = pathlib.Path("/etc/lu-c4-vm-role")


def require(ok, reason):
    if not ok:
        raise RuntimeError(reason)


def audit():
    require(sys.platform == "linux", "NOT_LINUX")
    require(pathlib.Path("/proc/1/comm").read_text().strip() == "systemd",
            "PID1_IS_NOT_SYSTEMD")
    detect = subprocess.run(["systemd-detect-virt", "--vm"],
                            capture_output=True, text=True, timeout=5)
    require(detect.returncode == 0 and detect.stdout.strip()
            not in ("", "none"), "NOT_A_REAL_VM")
    hypervisor = detect.stdout.strip()

    rows = pathlib.Path("/proc/self/cgroup").read_text().splitlines()
    require(len(rows) == 1 and rows[0].startswith("0::"),
            "UNIFIED_CGROUP_V2_MISSING")
    mount_lines = pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
    cgroup_mounts = [m for m in mount_lines
                     if " - cgroup2 " in m and
                     m.split(" - ", 1)[0].split()[4] == "/sys/fs/cgroup"]
    require(len(cgroup_mounts) == 1, "NOT_NATIVE_CGROUP2_MOUNT")
    mount_flags = cgroup_mounts[0].split(" - ", 1)[0].split()[5].split(",")
    require("rw" in mount_flags, "CGROUP_V2_MOUNT_READ_ONLY")
    require((pathlib.Path("/sys/fs/cgroup") /
             "cgroup.controllers").is_file(),
            "CGROUP_CONTROLLERS_NOT_IN_KERNEL")

    role_meta = ROLE.lstat()
    require(stat.S_ISREG(role_meta.st_mode) and role_meta.st_uid == 0,
            "MISSING_ROOT_OWNED_DISPOSABLE_VM_MARKER")
    role = ROLE.read_text()
    require(f"originalCoreSha={SHA}" in role and
            "githubRunnerRegistration=NOT_AUTHORIZED_BEFORE_OS_PREFLIGHT" in role,
            "WRONG_VM_SOURCE_IDENTITY")

    worker = pwd.getpwnam("lu-c4")
    require(worker.pw_uid != 0, "ROOT_WORKER_NOT_ALLOWED")
    memberships = subprocess.check_output(
        ["id", "-nG", "lu-c4"], text=True, timeout=5
    ).split()
    require("sudo" not in memberships and "adm" not in memberships,
            "WORKER_HAS_ADMIN_GROUP_ACCESS")

    path = ROOT / "runner"
    metadata = path.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and
            metadata.st_uid == worker.pw_uid and
            stat.S_IMODE(metadata.st_mode) == 0o700,
            "RUNNER_DIRECTORY_NOT_PRIVATE")
    require(not any((path / name).exists() for name in
                    (".runner", ".credentials", ".credentials_rsaparams")),
            "PREEXISTING_OR_PERSISTENT_RUNNER_REGISTRATION")
    require((ROOT / "lu-main").is_dir(), "MISSING_ADMIN_REVIEWED_REPO_STAGING")

    for command in ("git", "python3", "node", "cmake", "cc",
                    "systemd-run", "sha256sum"):
        require(shutil.which(command) is not None,
                "NATIVE_TOOL_MISSING_" + command.replace("-", "_"))

    return {
        "status": "VM_BOOTSTRAP_AUDIT_ONLY",
        "originalCoreSha": SHA,
        "hypervisor": hypervisor,
        "pid1": "systemd",
        "unifiedCgroupV2": True,
        "mountWritable": True,
        "runnerUserNonroot": True,
        "runnerHasSudoGroup": False,
        "runnerDirectoryPrivate": True,
        "networkBoundaryVerified": False,  # requires independent host firewall
        "systemdDelegationVerified": False, # requires running Lu's OS preflight
        "githubRunnerRegistered": False,
        "originalKernelCgroupReceipt": False,
        "releaseAuthorized": False,
    }


def main():
    try:
        result = audit()
        code = 0
    except (OSError, RuntimeError, subprocess.SubprocessError,
            KeyError, ValueError) as exc:
        result = {
            "status": "BLOCKED",
            "reason": str(exc),
            "originalCoreSha": SHA,
            "networkBoundaryVerified": False,
            "systemdDelegationVerified": False,
            "githubRunnerRegistered": False,
            "originalKernelCgroupReceipt": False,
            "releaseAuthorized": False,
        }
        code = 78
    print(json.dumps(result, indent=2, sort_keys=True))
    return code


if __name__ == "__main__":
    sys.exit(main())
