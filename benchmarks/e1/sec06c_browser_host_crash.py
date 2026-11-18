#!/usr/bin/python3
"""SEC-06C original Linux Browser host-SIGKILL and restart custody test.

NOT a source-only simulation. The parent verifier SIGKILLs the real first-
party Python Lu host AFTER checking distinct actual headed Chromium and Node
PIDs in one root-owned systemd cgroup leaf. The system supervisor must detect
the real lifetime-pipe EOF, kill the kernel subtree, durably fsync the exact
root receipt and leave an unrelated process alive. No old authorization replay.
"""
import argparse
import hashlib
import json
import os
import pathlib
import select
import signal
import subprocess
import sys
import tempfile
import time
from sec06c_browser_real_native import (UID,CHAT,TURN,TASK,ORIGINS,sha,
    root_receipt,live,membership,exact_line,reject)

def original_command(bundle,launcher,nonce,profile,mode):
    chrome=bundle/"chrome"
    node=bundle/"node"
    script=bundle/"lu_browser_worker.mjs"
    hashes=[sha(chrome),sha(node),sha(script)]
    policy=hashlib.sha256(("\0".join([str(profile),mode]+ORIGINS)+"\0").encode()).hexdigest()
    cmd=["sudo","-n",str(launcher),"--nonce",nonce,"--chat",CHAT,
         "--turn",TURN,"--call",TASK,
         "--chromium",str(chrome),"--chromium-sha",hashes[0],
         "--node",str(node),"--node-sha",hashes[1],
         "--script",str(script),"--script-sha",hashes[2],
         "--profile",str(profile),"--policy-sha",policy,
         "--mode",mode,"--display",os.environ["DISPLAY"],
         "--origin",ORIGINS[0]]
    if os.environ.get("XAUTHORITY"):
        cmd+=["--xauthority",
              str(pathlib.Path(os.environ["XAUTHORITY"]).resolve(strict=True))]
    return cmd,hashes,policy
def write_durable(file,record):
    fd=os.open(file,os.O_CREAT|os.O_EXCL|os.O_WRONLY|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,"w",encoding="ascii") as out:
        json.dump(record,out,sort_keys=True)
        out.write("\n");out.flush();os.fsync(out.fileno())
    folder=os.open(file.parent,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(folder)
    finally:os.close(folder)
def child_host(a):
    # This host is intentionally SIGKILLed by the independent verifier.
    cmd,hashes,policy=original_command(
        pathlib.Path(a.bundle),pathlib.Path(a.launcher),
        a.nonce,pathlib.Path(a.profile),a.mode)
    broker=subprocess.Popen(cmd,stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,stderr=subprocess.PIPE,close_fds=True)
    line=exact_line(broker,timeout=45,limit=256)
    fields=line.strip().split("\t")
    if len(fields)!=5 or fields[:2]!=["LU_SEC06C_BROWSER_READY_V1",a.nonce]:
        reject("original root browser supervisor did not admit both peers")
    chrome,node,port=map(int,fields[2:])
    if chrome<=1 or node<=1 or chrome==node or not 1<=port<=65535:
        reject("original browser PIDFD identity not valid")
    group=membership(chrome)
    if group!=membership(node) or not group.endswith(
        "lu-sec06c-browser-"+a.nonce+".service/lu-sec06c-leaf-"+a.nonce):
        reject("original Chromium and Node not jointly confined")
    write_durable(pathlib.Path(a.evidence),{
        "nonce":a.nonce,"host":os.getpid(),"broker":broker.pid,
        "chromium":chrome,"node":node,"cgroup":group,"sha":hashes,
        "policy":policy,"chat":CHAT,"turn":TURN,"task":TASK,
    })
    while True:time.sleep(3600)
def waiting_for(path,host,deadline):
    while time.monotonic()<deadline:
        if path.exists():return json.loads(path.read_text(encoding="ascii"))
        if host.poll() is not None:reject("first-party original host died before test SIGKILL")
        time.sleep(.03)
    reject("original browser pair never became kernel-admitted before host death")
def confirm_terminated(pid,pidfd,deadline):
    poller=select.poll()
    poller.register(pidfd,select.POLLIN)
    while time.monotonic()<deadline:
        if poller.poll(0) or not live(pid):return
        time.sleep(.03)
    reject("original browser process still running after Lu host SIGKILL")
def verify_host_loss(a):
    if sys.platform!="linux" or UID==0:
        reject("requires genuine unprivileged Linux original host")
    if not os.getenv("DISPLAY"):reject("genuine Xvfb DISPLAY unavailable")
    bundle=pathlib.Path(a.bundle).resolve(strict=True)
    launcher=pathlib.Path(a.launcher).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="lu-sec06c-browser-sigkill-") as tmp:
        root=pathlib.Path(tmp)
        profile=root/"profile"
        profile.mkdir()
        nonce=os.urandom(16).hex()
        evidence=root/"original-ready.json"
        receipt=pathlib.Path("/var/lib/lu-sec06c/browser-receipts")/(nonce+".receipt")
        if receipt.exists():reject("one-use root browser nonce replay before test")
        mode="always-visible"
        host=subprocess.Popen([sys.executable,str(pathlib.Path(__file__).resolve()),
            "--host","--bundle",str(bundle),"--launcher",str(launcher),
            "--profile",str(profile),"--nonce",nonce,"--mode",mode,
            "--evidence",str(evidence)],
            stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,close_fds=True)
        control=subprocess.Popen(["/bin/sleep","80"],stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        fds=[]
        try:
            recorded=waiting_for(evidence,host,time.monotonic()+52)
            if recorded["host"]!=host.pid or recorded["nonce"]!=nonce:
                reject("original first-party host and service identities diverged")
            chrome=int(recorded["chromium"]);node=int(recorded["node"])
            if chrome<=1 or node<=1 or chrome==node:
                reject("original OS browser pair pid invalid")
            fds=[os.pidfd_open(chrome),os.pidfd_open(node)]
            group=recorded["cgroup"]
            leaf=pathlib.Path("/sys/fs/cgroup")/group.removeprefix("0::/")
            if not leaf.is_dir() or not (leaf/"cgroup.kill").is_file():
                reject("no original real Linux cgroup kill subtree")
            if not live(chrome) or not live(node):
                reject("original browser died before creator host SIGKILL")
            os.kill(host.pid,signal.SIGKILL)
            host.wait(timeout=5)
            if host.returncode!=-signal.SIGKILL:
                reject("creator host did not actually receive SIGKILL")
            deadline=time.monotonic()+60
            while time.monotonic()<deadline and not receipt.exists():
                time.sleep(.04)
            if not receipt.exists():
                reject("root supervisor did not fsync browser cleanup after host death")
            root_receipt(receipt,nonce,*recorded["sha"],recorded["policy"])
            for pid,pidfd in zip((chrome,node),fds):
                confirm_terminated(pid,pidfd,deadline)
            if leaf.exists():
                reject("original root-owned browser leaf survived host SIGKILL")
            if control.poll() is not None:
                reject("unrelated user process was killed by browser supervisor")
            # The same original bearer task/nonce may never launch again.
            cmd,_,_=original_command(bundle,launcher,nonce,profile,mode)
            retry=subprocess.run(cmd,input=b"",capture_output=True,timeout=20)
            if retry.returncode==0:
                reject("old browser session replay succeeded after original host death")
            print("SEC06C_BROWSER_ORIGINAL_HOST_SIGKILL_PASS=1 "
                "genuine_chromium_node=1 single_root_owned_leaf=1 "
                "original_lu_host_sigkill=1 scoped_cgroup_kill=1 "
                "fsynced_cleanup_receipt=1 unrelated_process_survived=1 "
                "restart_replay_denied=1",flush=True)
        finally:
            for fd in fds:os.close(fd)
            if host.poll() is None:
                host.kill();host.wait(timeout=5)
            if control.poll() is None:control.terminate()
            control.wait(timeout=5)
    return 0
def main():
    p=argparse.ArgumentParser(allow_abbrev=False)
    p.add_argument("--bundle",required=True)
    p.add_argument("--launcher",required=True)
    p.add_argument("--host",action="store_true")
    p.add_argument("--nonce")
    p.add_argument("--profile")
    p.add_argument("--mode")
    p.add_argument("--evidence")
    a=p.parse_args()
    if a.host:
        if (not a.nonce or not a.profile or not a.mode or not a.evidence):
            reject("exact original child-host fixture identity missing")
        return child_host(a)
    return verify_host_loss(a)
if __name__=="__main__":
    try:sys.exit(main())
    except Exception as exc:
        print("SEC06C_BROWSER_ORIGINAL_HOST_LOSS_FAILED: "+str(exc),file=sys.stderr)
        sys.exit(93)
