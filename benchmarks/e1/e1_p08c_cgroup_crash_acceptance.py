#!/usr/bin/env python3
"""E1-P08C independent Linux kernel cgroup-v2 acceptance.

Run as an UNPRIVILEGED user whose unit has *already* been delegated by
systemd or a compatible cgroup v2 manager. CI may use root to CREATE
the delegated service, but this test and all Lu helpers must run as the
ordinary user. No simulated kernel answers, PID-based cleanup, or fake
cgroup.kill implementation are accepted.

The parent process of the worker is SIGKILLed while the independent,
host-spawned cgroup guard remains OUTSIDE the worker subgroup.
The test proves that a live setsid() descendant dies and cgroup.events
reaches recursive populated 0, without killing the unrelated control.
"""
import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


def state(pid: int):
    try:
        line = Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
        return line.rsplit(")", 1)[1].split()[0]
    except FileNotFoundError:
        return None


def living(pid: int) -> bool:
    return state(pid) not in (None, "Z", "X", "x")


def kernel_membership(pid: int) -> str:
    for line in Path(f"/proc/{pid}/cgroup").read_text(encoding="ascii").splitlines():
        if line.startswith("0::"):
            return line[3:]
    raise RuntimeError(f"missing real cgroup-v2 membership for {pid}")


def events(path: Path) -> dict[str, str]:
    raw = (path / "cgroup.events").read_text(encoding="ascii")
    return dict(line.split(maxsplit=1) for line in raw.splitlines())


def main() -> None:
    if sys.platform != "linux":
        raise RuntimeError("requires Linux")
    parser = argparse.ArgumentParser()
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--guard", type=Path, required=True)
    args = parser.parse_args()
    if not all(p.is_file() for p in (args.parent, args.guard)):
        raise RuntimeError("missing actual native first-party binaries")
    if os.geteuid() == 0:
        raise RuntimeError("test may not run as root: delegation must be real")
    with tempfile.TemporaryDirectory(prefix="lu-e1-p08c-") as directory:
        receipt = Path(directory) / "worker_pids.txt"
        env = os.environ.copy()
        env["LU_E1_P08C_TEST_GUARD_BINARY"] = str(args.guard.resolve())
        control = subprocess.Popen(["/bin/sleep", "90"], stdin=subprocess.DEVNULL)
        parent = subprocess.Popen(
            [str(args.parent.resolve()), str(receipt)],
            env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        pinned: dict[int, int] = {}
        guardian_pid = worker_pid = descendant_pid = None
        leaf = None
        try:
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                if parent.poll() is not None:
                    raise RuntimeError(
                        f"cgroup guardian/worker admission denied ({parent.returncode}); "
                        "not a passing native test: delegation may be unavailable"
                    )
                if receipt.exists():
                    items = receipt.read_text(encoding="ascii").split()
                    if len(items) == 3:
                        leaf = Path(items[0])
                        worker_pid, descendant_pid = map(int, items[1:])
                        guardian_pid = int(leaf.name.rsplit("-", 1)[1])
                        break
                time.sleep(0.04)
            if not all((leaf, guardian_pid, worker_pid, descendant_pid)):
                raise RuntimeError("no native cgroup readiness + fork evidence")
            ids = (guardian_pid, worker_pid, descendant_pid)
            if any(pid <= 1 for pid in ids):
                raise RuntimeError("invalid native process identity")
            if not leaf.is_dir() or not leaf.name.startswith("lu-e1-custody-"):
                raise RuntimeError("kernel worker leaf not present")
            if events(leaf).get("populated") != "1":
                raise RuntimeError("kernel cgroup not populated before host crash")
            actual = {p: kernel_membership(p) for p in ids}
            expected_suffix = "/" + leaf.name
            if not actual[worker_pid].endswith(expected_suffix):
                raise RuntimeError("worker executed before cgroup admission")
            if not actual[descendant_pid].endswith(expected_suffix + "/lu-e1-nested"):
                raise RuntimeError("setsid child was not admitted into nested worker cgroup")
            if events(leaf / "lu-e1-nested").get("populated") != "1":
                raise RuntimeError("nested cgroup lacked a live setsid descendant")
            if actual[guardian_pid].endswith(expected_suffix):
                raise RuntimeError("guardian wrongly placed inside killable subtree")
            if actual[guardian_pid] != kernel_membership(parent.pid):
                raise RuntimeError("guardian not in the original delegated owner scope")
            if os.getpgid(worker_pid) == os.getpgid(descendant_pid):
                raise RuntimeError("setsid negative control did not change process group")
            if not living(control.pid) or kernel_membership(control.pid) != actual[guardian_pid]:
                raise RuntimeError("unrelated control not in sibling parent cgroup")
            # Pin exact identities before injecting SIGKILL; failure cleanup
            # never uses stale PIDs/PGIDs obtained from an interrupted run.
            pinned = {pid: os.pidfd_open(pid) for pid in ids}
            print(
                "E1_P08C_BEFORE "
                f"guard={guardian_pid} worker={worker_pid} setsid={descendant_pid} "
                f"control={control.pid} populated=1 "
                "guard_outside=1 pre_exec_cgroup=1 nested_cgroup=1",
                flush=True,
            )
            os.kill(parent.pid, signal.SIGKILL)
            parent.wait(timeout=5)
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                if not leaf.exists() and not any(living(pid) for pid in ids):
                    break
                time.sleep(0.04)
            statuses = {pid: state(pid) for pid in ids}
            print("E1_P08C_AFTER " + repr(statuses), flush=True)
            if leaf.exists():
                raise RuntimeError(
                    "independent guardian did not get cgroup.kill populated=0 "
                    "and remove the owned cgroup"
                )
            if any(living(pid) for pid in ids):
                raise RuntimeError("cgroup-v2 left a live in-subtree/guardian process")
            if not living(control.pid):
                raise RuntimeError("unrelated control process was killed")
            parent_migratable = os.access(str(leaf.parent / "cgroup.procs"), os.W_OK)
            print(
                f"E1_P08C_ANCESTOR_MIGRATION_RISK={int(parent_migratable)} "
                "full_adversarial_containment_not_claimed=1",
                flush=True,
            )
            print(
                "E1_P08C_SETSID_CGROUP_CUSTODY_PASS=1 "
                "setsid_escaped_pgid=1 setsid_still_in_cgroup=1 "
                "crashed_owner=1 guardian_outside=1 nested_killed=1 cgroup_empty=1 "
                "unrelated_alive=1",
                flush=True,
            )
        finally:
            if parent.poll() is None:
                parent.kill()
                parent.wait(timeout=5)
            # This failure cleanup uses only pinned kernel PIDs, never
            # a stale numeric cgroup path, PID or process group from disk.
            for fd in pinned.values():
                try:
                    signal.pidfd_send_signal(fd, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                finally:
                    os.close(fd)
            if control.poll() is None:
                control.terminate()
            control.wait(timeout=5)


if __name__ == "__main__":
    main()
