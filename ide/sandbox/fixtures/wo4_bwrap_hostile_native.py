#!/usr/bin/env python3
"""Independent P03 Bwrap hostile host-filesystem/network OS witness.

Test-only, no project path or executable arguments. Runs a first-party fixed
Python script inside the SAME no-network/unshare/read-only namespace shape used
by the Lu IDE LSP/LLDB workers. It cannot grant execution authority or approve
a release. No Docker, fake namespace or fallback.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

BWRAP = "/usr/bin/bwrap"
SENTINEL = "testonly-owned-project.txt"


def inside() -> int:
    if len(sys.argv) != 2 or sys.argv[1] != "--inside":
        return 2
    expected = os.environ.get("LU_IDE_P03_HOST_NET_NS", "")
    if not expected.startswith("net:["):
        return 2
    now = os.readlink("/proc/self/ns/net")
    if now == expected:
        return 2  # Kernel must prove a different network namespace.
    if Path("/home/runner").exists() or Path("/etc/shadow").exists():
        return 2
    source = Path("/workspace") / SENTINEL
    if not source.is_file() or source.read_text() != "P03-fixed-readonly-source":
        return 2
    # The approved workspace is read-only; no root-like fallback or filesystem
    # write to that protected source may succeed inside the user namespace.
    try:
        source.write_text("denied")
    except OSError:
        pass
    else:
        return 2
    # Kernel's process namespace is likewise distinct; no host PID mount.
    host_pid = os.environ.get("LU_IDE_P03_HOST_PID_NS", "")
    if not host_pid.startswith("pid:["):
        return 2
    if os.readlink("/proc/self/ns/pid") == host_pid:
        return 2
    return 0


def host() -> int:
    if len(sys.argv) != 1 or not Path(BWRAP).is_file():
        return 2
    source = Path(__file__).resolve()
    with tempfile.TemporaryDirectory(prefix="lu-ide-p03-bwrap-") as tmp:
        root = Path(tmp)
        (root / SENTINEL).write_text("P03-fixed-readonly-source")
        # The script lives with source and is itself sealed into the fixed
        # allowed workspace. Never execute a user-supplied program or path.
        (root / "wo4_bwrap_hostile_native.py").write_bytes(source.read_bytes())
        command = [
            BWRAP, "--die-with-parent", "--new-session", "--unshare-all",
            "--clearenv", "--ro-bind", "/usr", "/usr",
            "--ro-bind-try", "/bin", "/bin",
            "--ro-bind-try", "/lib", "/lib",
            "--ro-bind-try", "/lib64", "/lib64",
            "--ro-bind", str(root), "/workspace",
            "--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev",
            "--setenv", "HOME", "/tmp",
            "--setenv", "PATH", "/usr/bin:/bin",
            "--setenv", "LANG", "C",
            "--setenv", "LU_IDE_P03_HOST_NET_NS", os.readlink("/proc/self/ns/net"),
            "--setenv", "LU_IDE_P03_HOST_PID_NS", os.readlink("/proc/self/ns/pid"),
            "--chdir", "/workspace",
            "--", "/usr/bin/python3", "-I", "/workspace/wo4_bwrap_hostile_native.py", "--inside",
        ]
        try:
            result = subprocess.run(command, stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                timeout=20, check=False)
        except (OSError, subprocess.TimeoutExpired):
            print("DENIED: native P03 Bwrap isolation not verified")
            return 2
        if result.returncode != 0:
            print("DENIED: actual P03 Bwrap hostile OS witness failed")
            return 2
        if (root / SENTINEL).read_text() != "P03-fixed-readonly-source":
            print("DENIED: native witness modified source")
            return 2
        print("PASS: native Bwrap distinct kernel PID/network namespaces, no host files, sealed read-only workspace")
        return 0


if __name__ == "__main__":
    raise SystemExit(inside() if sys.argv[1:] == ["--inside"] else host())
