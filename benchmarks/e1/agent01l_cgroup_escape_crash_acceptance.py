#!/usr/bin/env python3
"""AGENT-01L native Linux cgroup-v2 host-crash and escaped-descendant test.

Positive success requires *actual delegated* kernel cgroup-v2 capabilities,
an independently running first-party custodian, a SIGKILL of Lu's parent,
and a separately pinned setsid grandchild that exits while an unrelated
control process survives. This script never escalates privileges or accepts
a fabricated success marker. NOT_AVAILABLE is exit code 78, never PASS.
"""
import argparse
import os
import pathlib
import select
import signal
import subprocess
import sys
import tempfile
import time

EXPECTED_CHAT="c-"+"1"*32
EXPECTED_TURN="g-"+"2"*32
EXPECTED_CALL="agent01l-cgroup-escaped-gh"


def state(pid):
    try:
        return pathlib.Path(f"/proc/{pid}/stat").read_text(
            encoding="ascii").rsplit(")", 1)[1].split()[0]
    except FileNotFoundError:
        return None


def running(pid):
    return state(pid) not in (None,"Z","X","x")


def poll_pidfd(pidfd):
    p=select.poll()
    p.register(pidfd,select.POLLIN)
    return bool(p.poll(0))


def read_evidence(path):
    if not path.exists():
        return None
    raw=path.read_text(encoding="ascii")
    if raw=="NOT_AVAILABLE\n":
        return {"status":"NOT_AVAILABLE"}
    lines=raw.splitlines()
    if len(lines)!=6 or lines[0]!="READY":
        raise RuntimeError("Invalid native host/cgroup evidence format")
    data={}
    for line in lines[1:]:
        k,sep,v=line.partition("=")
        if not sep or k in data:
            raise RuntimeError("Duplicate or malformed OS evidence")
        data[k]=v
    if set(data)!={"HOST","LEADER","ESCAPED","CGROUP","RECEIPT"}:
        raise RuntimeError("Missing source of kernel evidence")
    return data


def require_os_termination(pid,fd,deadline):
    while time.monotonic()<deadline:
        if poll_pidfd(fd) or not running(pid):
            return
        time.sleep(0.025)
    raise RuntimeError("Escaped descendant still running after cgroup.kill")


def main():
    if sys.platform!="linux":
        raise RuntimeError("Linux kernel-only acceptance")
    ap=argparse.ArgumentParser()
    ap.add_argument("--parent",required=True)
    ap.add_argument("--guard",required=True)
    args=ap.parse_args()
    parent=pathlib.Path(args.parent).resolve(strict=True)
    guard=pathlib.Path(args.guard).resolve(strict=True)
    if not os.access(parent,os.X_OK) or not os.access(guard,os.X_OK):
        raise RuntimeError("Native fixture/custodian must be executable")
    with tempfile.TemporaryDirectory(prefix="lu-agent01l-cgroup-") as d:
        root=pathlib.Path(d)
        evidence=root/"host-crash-evidence.txt"
        env=dict(os.environ,LU_AGENT01L_TEST_CGROUP_GUARD=str(guard))
        host=subprocess.Popen(
            [str(parent),str(root),str(evidence)],
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,env=env,
        )
        control=None
        escaped_fd=None
        escaped_pid=None
        try:
            deadline=time.monotonic()+12
            proof=None
            while time.monotonic()<deadline:
                proof=read_evidence(evidence)
                if proof is not None:
                    break
                if host.poll() is not None:
                    raise RuntimeError("Native custodian host exited early: "
                        +host.stderr.read(2000).decode(errors="replace"))
                time.sleep(.025)
            if proof is None:
                raise RuntimeError("No native cgroup delegation handshake")
            if proof.get("status")=="NOT_AVAILABLE":
                print("AGENT01L_CGROUP_DELEGATION_NOT_AVAILABLE=1 "
                      "positive_escape_custody=OPEN",flush=True)
                return 78
            if int(proof["HOST"])!=host.pid:
                raise RuntimeError("Fixture did not record exact live parent")
            escaped_pid=int(proof["ESCAPED"])
            leader=int(proof["LEADER"])
            escaped_fd=os.pidfd_open(escaped_pid)
            if not running(escaped_pid):
                raise RuntimeError("Escaped grandchild missing before host SIGKILL")
            if os.getpgid(leader)==os.getpgid(escaped_pid):
                raise RuntimeError("Escaped descendant never called setsid")
            if not proof["CGROUP"].startswith("/sys/fs/cgroup/"):
                raise RuntimeError("Unexpected kernel cgroup location")
            cgroup=pathlib.Path(proof["CGROUP"])
            if not cgroup.joinpath("cgroup.kill").exists():
                raise RuntimeError("No kernel cgroup.kill")
            leader_cgroup=pathlib.Path(f"/proc/{leader}/cgroup").read_text()
            escaped_cgroup=pathlib.Path(
                f"/proc/{escaped_pid}/cgroup").read_text()
            expected_nested=leader_cgroup.strip()+"/lu-nested-host-test"
            if escaped_cgroup.strip()!=expected_nested:
                raise RuntimeError("Nested setsid child did not remain inside trusted subtree")
            nested=cgroup/"lu-nested-host-test"
            if not nested.is_dir():
                raise RuntimeError("Nested child cgroup was not actually created")
            # Control is outside the killed group; proves that this is not an
            # indiscriminate kill of all processes owned by the user.
            control=subprocess.Popen(
                ["/bin/sleep","20"],stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            if not running(control.pid):
                raise RuntimeError("Unrelated process control never started")
            os.kill(host.pid,signal.SIGKILL)
            host.wait(timeout=5)
            if host.returncode!=-signal.SIGKILL:
                raise RuntimeError("Test Lu parent was not actually SIGKILLed")
            receipt=pathlib.Path(proof["RECEIPT"])
            end=time.monotonic()+13
            while time.monotonic()<end:
                if receipt.exists():
                    break
                time.sleep(.03)
            if not receipt.exists():
                raise RuntimeError(
                    "No durable kernel cgroup.events populated-zero receipt after Lu death"
                )
            raw=receipt.read_text(encoding="ascii")
            rows=raw.splitlines()
            if len(rows)!=6 or rows[0]!="LU_CGROUP_V2_EMPTY_V1" or (
                rows[1]!=f"CHAT={EXPECTED_CHAT}" or
                rows[2]!=f"TURN={EXPECTED_TURN}" or
                rows[3]!=f"CALL={EXPECTED_CALL}" or
                rows[5]!="POPULATED=0"
            ):
                raise RuntimeError("Custodian OS receipt origin/emptiness invalid")
            nonce=rows[4].removeprefix("NONCE=")
            if not rows[4].startswith("NONCE=") or len(nonce)!=32 or (
                any(c not in "0123456789abcdef" for c in nonce)
            ):
                raise RuntimeError("Missing independently generated custody ID")
            require_os_termination(escaped_pid,escaped_fd,end)
            if not running(control.pid):
                raise RuntimeError("Unrelated control process was killed")
            # Strong P08C parity: nested child and owner leaf must both be
            # removed after cgroup.kill and recursive populated=0.
            if nested.exists() or cgroup.exists():
                raise RuntimeError("Custodian did not remove its empty nested cgroup tree")
            print("AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1 "
                "Lu_SIGKILL=1 setsid_escape=1 "
                "cgroup_kill=1 recursive_populated_zero=1 "
                "unrelated_process_survived=1 original_scope_receipt=1",
                flush=True)
            return 0
        finally:
            if host.poll() is None:
                host.kill()
                host.wait(timeout=5)
            if escaped_fd is not None:
                if not poll_pidfd(escaped_fd):
                    try:
                        signal.pidfd_send_signal(escaped_fd,signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                os.close(escaped_fd)
            if control is not None:
                if control.poll() is None:
                    control.terminate()
                control.wait(timeout=5)


if __name__=="__main__":
    sys.exit(main())
