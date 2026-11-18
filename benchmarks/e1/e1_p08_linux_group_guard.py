#!/usr/bin/env python3
"""Native Linux E1-P08 proof: SIGKILL the Lu-like host and observe descendants.

Tests an actual first-party group guardian, OS shell child, sleep grandchild,
and unrelated control process. No mocked kernel result or SQLite inference.
The guardian ACK is kernel SIGKILL; adopted zombies may require init to reap.
"""
import argparse
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def state(pid: int):
    try:
        line = Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
        return line.rsplit(")", 1)[1].split()[0]
    except FileNotFoundError:
        return None


def live(pid: int) -> bool:
    return state(pid) not in (None, "Z", "X", "x")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--guard", type=Path, required=True)
    args = parser.parse_args()
    if not args.parent.is_file() or not args.guard.is_file():
        raise RuntimeError("Actual native Linux executable not staged")
    with tempfile.TemporaryDirectory(prefix="lu-e1-p08-linux-os-") as directory:
        receipt = Path(directory, "real_pids.txt")
        env = os.environ.copy()
        env["LU_E1_P08_TEST_GUARD_BINARY"] = str(args.guard.resolve())
        control = subprocess.Popen(["/bin/sleep", "90"])
        parent = subprocess.Popen(
            [str(args.parent.resolve()), str(receipt)],
            env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
        pgid = worker = descendant = None
        owned_pidfds = {}
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if parent.poll() is not None:
                    raise RuntimeError("Lu-like host exited unexpectedly before setup")
                if receipt.exists():
                    parts = receipt.read_text(encoding="ascii").split()
                    if len(parts) == 3:
                        pgid, worker, descendant = map(int, parts)
                        break
                time.sleep(0.05)
            if not all((pgid, worker, descendant)):
                raise RuntimeError("Native child and grandchild did not start")
            if pgid <= 1 or worker <= 1 or descendant <= 1:
                raise RuntimeError("Unsafe process identities in test")
            if os.getpgid(worker) != pgid or os.getpgid(descendant) != pgid:
                raise RuntimeError("Descendants were never in guardian-owned PGID")
            if not live(pgid) or not live(worker) or not live(descendant):
                raise RuntimeError("Group members disappeared before crash injection")
            if not live(control.pid) or os.getpgid(control.pid) == pgid:
                raise RuntimeError("Control process was not independently owned")
            # Pin each tested OS PID identity *before* killing the Lu host.
            # Cleanup can then signal only those exact processes, never
            # a recycled process-group number recovered from a file.
            owned_pidfds = {pid: os.pidfd_open(pid)
                            for pid in (pgid, worker, descendant)}
            print(f"E1_P08_KERNEL_BEFORE guardian={pgid} worker={worker} descendant={descendant} control={control.pid}", flush=True)
            os.kill(parent.pid, signal.SIGKILL)
            parent.wait(timeout=5)
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                if not any(live(pid) for pid in (pgid, worker, descendant)):
                    break
                time.sleep(0.05)
            statuses = {name: state(pid) for name, pid in (
                ("guardian", pgid), ("worker", worker), ("descendant", descendant)
            )}
            print("E1_P08_KERNEL_AFTER " + str(statuses), flush=True)
            if any(live(pid) for pid in (pgid, worker, descendant)):
                raise RuntimeError("An owned group process remained alive after host SIGKILL")
            if not live(control.pid):
                raise RuntimeError("Unrelated control process was terminated")
            print("E1_P08_LINUX_ORPHAN_GUARD_PASS=1", flush=True)
        finally:
            if parent.poll() is None:
                parent.kill()
                parent.wait(timeout=5)
            # On failure, terminate exact kernel-pinned identities only.
            # Never killpg a stale PID copied from the fixture file.
            for fd in owned_pidfds.values():
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
