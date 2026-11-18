#!/usr/bin/python3
"""SEC-06C browser root-manager-loss destructive fixture (Linux CI only).

An independent CI verifier proves that killing only the original verified
root-owned service main process cannot mint a false kernel-empty receipt.
systemd KillMode=control-group must terminate the separately pidfd-pinned
original Chromium and Node, including their descendant subtree. No arbitrary
PID or unrelated service is ever selected, and this fixture is NOT shipped.
"""
import argparse
import os
import pathlib
import select
import signal
import stat
import subprocess
import sys
import tempfile
import time
from sec06c_browser_real_native import (UID,sha,live,membership,exact_line,reject)
from sec06c_browser_host_crash import original_command

def signalled(fd):
    poller=select.poll();poller.register(fd,select.POLLIN)
    return bool(poller.poll(0))
def until_signalled(pid,fd,deadline):
    while time.monotonic()<deadline:
        if signalled(fd) or not live(pid):return
        time.sleep(.03)
    reject("systemd-owned browser original PID survived root-manager loss")
def find_exact_root_manager(unit_dir,nonce):
    expected="lu-sec06c-browser-"+nonce+".service"
    if unit_dir.name!=expected or unit_dir.is_symlink():
        reject("root-owned manager unit not original one-use service")
    meta=unit_dir.stat()
    if meta.st_uid!=0 or meta.st_mode&0o022:
        reject("original service parent became user-delegated or writable")
    contents=(unit_dir/"cgroup.procs").read_text().splitlines()
    match=[]
    for item in contents:
        if not item.isdecimal():reject("non-kernel service manager PID")
        pid=int(item)
        cmdline=pathlib.Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
        if b"/usr/libexec/lu/lu-sec06c-owned-browser-manager" in cmdline:
            meta=pathlib.Path(f"/proc/{pid}").stat()
            if meta.st_uid!=0:reject("root manager no longer privileged OS-only")
            match.append(pid)
    if len(match)!=1:reject("not exactly one original root-owned kernel browser manager")
    return match[0]
def original_manager_loss(bundle,launcher):
    if sys.platform!="linux" or UID==0 or not os.getenv("DISPLAY"):
        reject("requires real unprivileged Xvfb Linux test")
    with tempfile.TemporaryDirectory(prefix="lu-sec06c-browser-guardian-loss-") as directory:
        profile=pathlib.Path(directory)/"profile"
        profile.mkdir()
        nonce=os.urandom(16).hex()
        cmd,hashes,policy=original_command(bundle,launcher,nonce,profile,"always-visible")
        receipt=pathlib.Path("/var/lib/lu-sec06c/browser-receipts")/(nonce+".receipt")
        if receipt.exists():reject("old browser nonce receipt already present")
        client=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,close_fds=True)
        control=subprocess.Popen(["/bin/sleep","70"],stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        fds=[]
        try:
            banner=exact_line(client,timeout=48,limit=256)
            fields=banner.strip().split("\t")
            if len(fields)!=5 or fields[:2]!=["LU_SEC06C_BROWSER_READY_V1",nonce]:
                reject("cannot attack absent original joint root-owned browser pair")
            chrome,node,port=map(int,fields[2:])
            if chrome<=1 or node<=1 or chrome==node or not 1<=port<=65535:
                reject("real original browser kernel identities invalid")
            original=membership(chrome)
            if original!=membership(node):
                reject("Chromium and Node never shared original kernel service")
            leaf=pathlib.Path("/sys/fs/cgroup")/original.removeprefix("0::/")
            if not leaf.is_dir() or not (leaf/"cgroup.kill").is_file():
                reject("original root-owned kernel leaf missing")
            unit=leaf.parent
            manager=find_exact_root_manager(unit,nonce)
            if not live(manager) or not live(chrome) or not live(node):
                reject("original manager or browser exited before negative attack")
            fds=[os.pidfd_open(p) for p in (manager,chrome,node)]
            # CI-host root acts only on the specific already-created service
            # identity established by the original nonce and /proc. The
            # production app has NO arbitrary systemctl/kill permission.
            killing=subprocess.run(["sudo","-n","/usr/bin/systemctl","kill",
                "--kill-whom=main","--signal=SIGKILL",unit.name],
                capture_output=True,text=True,timeout=15)
            if killing.returncode!=0:
                reject("CI failed to SIGKILL exact root manager: "+killing.stderr[:600])
            if not signalled(fds[0]):
                deadline=time.monotonic()+10
                until_signalled(manager,fds[0],deadline)
            deadline=time.monotonic()+35
            for pid,fd in ((chrome,fds[1]),(node,fds[2])):
                until_signalled(pid,fd,deadline)
            if receipt.exists():
                reject("DEAD ROOT SUPERVISOR MINTED FALSE KERNEL_EMPTY=1 RECEIPT")
            if control.poll() is not None:
                reject("unrelated process was killed by exact service cleanup")
            if unit.exists() and (unit/"cgroup.events").is_file():
                records=(unit/"cgroup.events").read_text().splitlines()
                if "populated 1" in records:
                    reject("systemd kept original root-owned browser descendants")
            # The interrupted first-party session may never grant a positive
            # custody witness or permit replay with the old nonce.
            try:client.stdin.close()
            except (OSError,ValueError):pass
            try:client.wait(timeout=15)
            except subprocess.TimeoutExpired:
                reject("orphan original service client survived manager SIGKILL")
            replay=subprocess.run(cmd,input=b"",capture_output=True,timeout=20)
            if replay.returncode==0:
                reject("old original root-manager-loss nonce replayed successfully")
            print("SEC06C_BROWSER_ORIGINAL_GUARDIAN_LOSS_FAIL_CLOSED_PASS=1 "
                "original_root_manager_sigkill=1 distinct_chromium_node_pidfds=1 "
                "systemd_control_group_reaped=1 false_receipt_denied=1 "
                "stale_authority_replay_denied=1 unrelated_process_survived=1",
                flush=True)
        finally:
            for fd in fds:os.close(fd)
            if client.poll() is None:
                try:client.stdin.close()
                except (OSError,ValueError,AttributeError):pass
                try:client.wait(timeout=6)
                except subprocess.TimeoutExpired:client.kill();client.wait(timeout=5)
            if control.poll() is None:control.terminate()
            control.wait(timeout=5)
    return 0
def main():
    p=argparse.ArgumentParser(allow_abbrev=False)
    p.add_argument("--bundle",required=True)
    p.add_argument("--launcher",required=True)
    a=p.parse_args()
    return original_manager_loss(
        pathlib.Path(a.bundle).resolve(strict=True),
        pathlib.Path(a.launcher).resolve(strict=True))
if __name__=="__main__":
    try:sys.exit(main())
    except Exception as e:
        print("SEC06C_BROWSER_GUARDIAN_LOSS_NOT_VERIFIED: "+str(e),file=sys.stderr)
        sys.exit(93)
