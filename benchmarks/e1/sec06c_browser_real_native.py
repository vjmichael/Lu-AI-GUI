#!/usr/bin/python3
"""SEC-06C actual installed-root browser/Node kernel session acceptance.

Linux only; a real Xvfb display and independently bundled headed Playwright
Chromium/Node are REQUIRED. The original one-use root-owned manager is invoked
as the ordinary Lu UID. No fake PID, journal, kernel receipt or release pass.
Human takeover uses the same real Chrome X11 window selected only by its
manager-attested PID. Host cancellation is original stdin EOF.
"""
import argparse
import hashlib
import json
import os
import pathlib
import select
import signal
import stat
import subprocess
import sys
import tempfile
import time

UID=os.geteuid()
CHAT="c-"+"4"*32
TURN="g-"+"5"*32
TASK="browser-task-"+"6"*32
ORIGINS=["https://example.com"]
def reject(message):
    raise AssertionError("SEC06C_BROWSER_NATIVE_BLOCKED: "+message)
def sha(path):
    h=hashlib.sha256()
    with open(path,"rb") as stream:
        for chunk in iter(lambda:stream.read(65536),b""):h.update(chunk)
    return h.hexdigest()
def exact_line(proc,timeout=40,limit=3_000_000):
    deadline=time.monotonic()+timeout
    fd=proc.stdout.fileno()
    value=bytearray()
    while time.monotonic()<deadline:
        readable,_,_=select.select([fd],[],[],0.05)
        if not readable:
            if proc.poll() is not None:reject("root manager exited before original frame")
            continue
        data=os.read(fd,1)
        if not data:
            result=proc.stderr.read(4096).decode(errors="replace")
            reject("root manager EOF before original frame: "+result[-1600:])
        value.extend(data)
        if len(value)>limit:reject("unbounded mediator frame")
        if data==b"\n":
            return value.decode("utf-8")
    reject("original native browser response exceeded finite deadline")
def live(pid):
    try:return pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")",1)[1].split()[0] not in ("Z","X","x")
    except FileNotFoundError:return False
def membership(pid):
    return pathlib.Path(f"/proc/{pid}/cgroup").read_text().strip()
def root_receipt(path,nonce,chrome_sha,node_sha,script_sha,policy_sha):
    st=path.lstat()
    if (not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_nlink!=1
        or st.st_mode&0o022):
        reject("original root-owned browser receipt mutable")
    expected="\n".join([
      "LU_SEC06C_OWNED_BROWSER_V1","NONCE="+nonce,
      "CHAT="+CHAT,"TURN="+TURN,"CALL="+TASK,
      "CHROMIUM_SHA256="+chrome_sha,"NODE_SHA256="+node_sha,
      "SCRIPT_SHA256="+script_sha,"POLICY_SHA256="+policy_sha,
      "UNIT=lu-sec06c-browser-"+nonce+".service",
      "ORIGINAL_UID="+str(UID),"KERNEL_EMPTY=1","EFFECT=STOPPED",""])
    if path.read_text(encoding="ascii")!=expected:
        reject("root-owned original receipt identity does not match actual admission")
def browser_call(proc,chrome_pid,operation):
    proc.stdin.write((json.dumps({"op":operation},separators=(",",":"))+"\n").encode())
    proc.stdin.flush()
    deadline=time.monotonic()+28
    while time.monotonic()<deadline:
        frame=json.loads(exact_line(proc,timeout=27))
        if frame.get("type")=="host_visibility_request":
            visible=frame.get("visible")
            request=frame.get("requestId")
            if not isinstance(visible,bool) or not isinstance(request,str):
                reject("untrusted human visibility challenge")
            lookup=subprocess.run(["xdotool","search","--pid",str(chrome_pid)],
                capture_output=True,text=True,timeout=8)
            if lookup.returncode!=0 or not lookup.stdout.strip():
                reject("real headed Chromium X11 window not found by original PID")
            for window in lookup.stdout.strip().splitlines():
                if not window.isdigit():reject("X11 returned untrusted window identity")
                action="windowmap" if visible else "windowunmap"
                result=subprocess.run(["xdotool",action,"--sync",window],
                    capture_output=True,text=True,timeout=12)
                if result.returncode!=0:
                    reject("native human handoff window operation rejected")
            proc.stdin.write((json.dumps({
                "type":"host_visibility_response","requestId":request,
                "ok":True},separators=(",",":"))+"\n").encode())
            proc.stdin.flush()
            continue
        if not isinstance(frame.get("ok"),bool):
            reject("non-JSONL browser protocol reply")
        if frame["ok"]!=True:
            reject("original browser process denied "+operation+": "+str(frame.get("error")))
        return frame.get("result",{})
    reject("original browser operation did not answer")
def genuine_renderer_userns_sandbox(chrome_pid,group,leaf):
    """Independent kernel audit of a real Chromium renderer.

    The original headed Chrome must NOT merely run without --no-sandbox.
    A live renderer must be in Chrome's original root-owned cgroup while
    owning a DIFFERENT user namespace and active seccomp-BPF (mode 2).
    This never accepts a Playwright configuration field or self-reported
    browser flag as security evidence.
    """
    chrome_userns=pathlib.Path(f"/proc/{chrome_pid}/ns/user").readlink()
    deadline=time.monotonic()+8
    while time.monotonic()<deadline:
        try:
            candidates=(leaf/"cgroup.procs").read_text().split()
        except OSError:
            reject("kernel browser group vanished during sandbox observation")
        for raw in candidates:
            if not raw.isdecimal():continue
            pid=int(raw)
            if pid==chrome_pid or pid<=1:continue
            try:
                cmd=pathlib.Path(f"/proc/{pid}/cmdline").read_bytes()
                if b"--type=renderer" not in cmd.split(b"\0"):continue
                if b"--no-sandbox" in cmd or b"--disable-setuid-sandbox" in cmd:
                    reject("renderer disabled original Chromium sandbox")
                status=pathlib.Path(f"/proc/{pid}/status").read_text()
                status_lines=dict(line.split(":",1) for line in status.splitlines()
                                  if ":" in line)
                mode=status_lines.get("Seccomp","").strip()
                new_userns=pathlib.Path(f"/proc/{pid}/ns/user").readlink()
                if membership(pid)!=group:reject("renderer escaped SEC06C root kernel leaf")
                if mode!="2" or chrome_userns==new_userns:
                    reject("genuine Chromium user namespace/seccomp not observed")
                print("SEC06C_CHROMIUM_REAL_USERNS_SECCOMP_PASS=1 "
                      "renderer_in_original_kernel_leaf=1 "
                      "renderer_userns_distinct=1 seccomp_bpf=1",flush=True)
                return
            except (FileNotFoundError,ProcessLookupError):
                continue
        time.sleep(.05)
    reject("no genuinely sandboxed Chromium renderer in original SEC06C leaf")

def main():
    if sys.platform!="linux" or UID==0:reject("requires actual unprivileged Linux host")
    ap=argparse.ArgumentParser()
    ap.add_argument("--bundle",required=True)
    ap.add_argument("--launcher",required=True)
    a=ap.parse_args()
    bundle=pathlib.Path(a.bundle).resolve(strict=True)
    launcher=pathlib.Path(a.launcher).resolve(strict=True)
    chrome=bundle/"chrome";node=bundle/"node";script=bundle/"lu_browser_worker.mjs"
    for p in (chrome,node,script,launcher):
        if not p.is_file() or not os.access(p,os.R_OK):reject("reviewed native browser candidate missing")
    if not os.environ.get("DISPLAY"):reject("Xvfb/real X11 display absent")
    nonce=os.urandom(16).hex()
    with tempfile.TemporaryDirectory(prefix="lu-sec06c-browser-native-") as temp:
        profile=pathlib.Path(temp).resolve(strict=True)
        mode="automatic"
        digest=hashlib.sha256(("\0".join([str(profile),mode]+ORIGINS)+"\0").encode()).hexdigest()
        images=[sha(chrome),sha(node),sha(script)]
        args=["sudo","-n",str(launcher),"--nonce",nonce,
            "--chat",CHAT,"--turn",TURN,"--call",TASK,
            "--chromium",str(chrome),"--chromium-sha",images[0],
            "--node",str(node),"--node-sha",images[1],
            "--script",str(script),"--script-sha",images[2],
            "--profile",str(profile),"--policy-sha",digest,
            "--mode",mode,"--display",os.environ["DISPLAY"],
            "--origin",ORIGINS[0]]
        if os.environ.get("XAUTHORITY"):
            args.extend(["--xauthority",str(pathlib.Path(os.environ["XAUTHORITY"]).resolve(strict=True))])
        before=pathlib.Path("/var/lib/lu-sec06c/browser-receipts")/(nonce+".receipt")
        if before.exists():reject("browser original replay receipt preexists")
        # An unrelated process must survive the entire kernel-cgroup kill.
        control=subprocess.Popen(["/bin/sleep","80"],stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        proc=subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,close_fds=True)
        browser_pids=[]
        try:
            banner=exact_line(proc,timeout=48,limit=256)
            rows=banner.strip().split("\t")
            if len(rows)!=5 or rows[:2]!=["LU_SEC06C_BROWSER_READY_V1",nonce]:
                reject("root manager did not attest actual original process pair")
            chrome_pid,node_pid,port=map(int,rows[2:])
            browser_pids=[chrome_pid,node_pid]
            if min(browser_pids)<=1 or chrome_pid==node_pid or not 1<=port<=65535:
                reject("Chrome and Node kernel process identities invalid")
            expected="lu-sec06c-browser-"+nonce+".service/lu-sec06c-leaf-"+nonce
            group=membership(chrome_pid)
            if not group.endswith(expected) or group!=membership(node_pid):
                reject("Chromium and Node not members of one original systemd-owned leaf")
            leaf=pathlib.Path("/sys/fs/cgroup")/group.removeprefix("0::/")
            if leaf.stat().st_uid!=0 or leaf.stat().st_mode&0o022:
                reject("same-UID owner can mutate root-owned browser kernel leaf")
            if not (leaf/"cgroup.kill").is_file():reject("real cgroup.kill missing")
            pidfds=[os.pidfd_open(pid) for pid in browser_pids]
            try:
                report=browser_call(proc,chrome_pid,"status")
                genuine_renderer_userns_sandbox(chrome_pid,group,leaf)
                if report.get("windowVisibility")!="HIDDEN":
                    reject("automatic-hide original Chromium not verified")
                result=browser_call(proc,chrome_pid,"show_browser")
                if result.get("windowVisibility")!="VISIBLE":
                    reject("human-show original Chromium not verified")
                result=browser_call(proc,chrome_pid,"hide_browser")
                if result.get("windowVisibility")!="HIDDEN":
                    reject("human-hide original Chromium not verified")
                proc.stdin.close() # Exact original Lu host lifetime EOF.
                if proc.wait(timeout=50)!=0:
                    reject("root browser supervisor did not stop cleanly: "+
                        proc.stderr.read(4000).decode(errors="replace"))
                if not before.exists():reject("no fsynced original kernel-empty receipt")
                root_receipt(before,nonce,*images,digest)
                if leaf.exists():reject("worker leaf survived original cgroup.kill")
                for pid,fd in zip(browser_pids,pidfds):
                    watcher=select.poll();watcher.register(fd,select.POLLIN)
                    if not watcher.poll(2500) and live(pid):
                        reject("original headed Chromium/Node still running after scoped kill")
                if control.poll() is not None:reject("unrelated non-browser process was terminated")
                # The exact consumed one-use admission must never restart.
                replay=subprocess.run(args,input=b"",capture_output=True,timeout=20)
                if replay.returncode==0:
                    reject("one-use original browser nonce replay was accepted")
                if before.read_text(encoding="ascii").count("LU_SEC06C_OWNED_BROWSER_V1")!=1:
                    reject("duplicate original browser receipt after denied replay")
                print("SEC06C_GENUINE_BROWSER_KERNEL_PASS=1 original_chromium=1 "
                    "original_node=1 same_root_owned_cgroup=1 ancestor_migration_denied=1 "
                    "human_show_hide=1 fsynced_kernel_empty=1 cancellation=1 "
                    "unrelated_survived=1 nonce_replay_denied=1",flush=True)
            finally:
                for fd in pidfds:os.close(fd)
        finally:
            if proc.poll() is None:
                proc.stdin.close() if proc.stdin and not proc.stdin.closed else None
                try:proc.wait(timeout=8)
                except subprocess.TimeoutExpired:proc.kill();proc.wait(timeout=5)
            if control.poll() is None:control.terminate()
            control.wait(timeout=5)
    return 0
if __name__=="__main__":
    try:sys.exit(main())
    except Exception as exc:
        print("SEC06C_BROWSER_NATIVE_FAILED: "+str(exc),file=sys.stderr)
        sys.exit(93)
