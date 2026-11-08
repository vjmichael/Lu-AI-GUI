#!/usr/bin/env python3
"""Linux kernel negative control for Lu E1-P08 process-group lifetime claims.

This intentionally creates ONE child that calls setsid(). Killing the host
process group MUST leave that child running. It is then cleaned up by an
identity-pinned pidfd, never by a guessed, previously persisted PID/PGID.
This is a kernel behavior test, not proof of Lu's current group guardian.
"""
import os
import signal
import subprocess
import sys
import time


CHILD_CODE = """import subprocess
child = subprocess.Popen(
    ['/bin/sleep', '30'], start_new_session=True,
    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL)
print(child.pid, flush=True)
child.wait()
"""


def state(pid):
    try:
        content = open(f"/proc/{pid}/stat", encoding="ascii").read()
        return content.rsplit(")", 1)[1].split()[0]
    except FileNotFoundError:
        return None


def alive(pid):
    return state(pid) not in (None, "Z", "X", "x")


def main():
    if sys.platform != "linux":
        raise RuntimeError("Linux-only kernel escape proof")
    leader = subprocess.Popen(
        [sys.executable, "-u", "-c", CHILD_CODE],
        start_new_session=True, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    )
    escaped = None
    pin = None
    try:
        escaped = int(leader.stdout.readline().strip())
        pin = os.pidfd_open(escaped)
        group = os.getpgid(leader.pid)
        escaped_group = os.getpgid(escaped)
        membership = [
            open(f"/proc/{pid}/cgroup", encoding="ascii").read().strip()
            for pid in (leader.pid, escaped)
        ]
        if group <= 1 or group == escaped_group:
            raise RuntimeError("Child did not escape original process group")
        if membership[0] != membership[1]:
            raise RuntimeError("setsid incorrectly interpreted as cgroup migration")
        if not alive(escaped):
            raise RuntimeError("Escaped child not alive before group termination")
        os.killpg(group, signal.SIGKILL)
        leader.wait(timeout=5)
        time.sleep(0.15)
        if not alive(escaped):
            raise RuntimeError("Escape negative-control child unexpectedly died")
        print(
            "E1_P08_GROUP_ESCAPE_PROVEN=1 "
            "same_cgroup=1 different_pgid=1 escaped_alive_after_killpg=1",
            flush=True,
        )
    finally:
        # A real kernel pidfd pins this exact target against PID reuse.
        if pin is not None:
            try:
                signal.pidfd_send_signal(pin, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(pin)
        if leader.poll() is None:
            leader.kill()
            leader.wait(timeout=5)


if __name__ == "__main__":
    main()
