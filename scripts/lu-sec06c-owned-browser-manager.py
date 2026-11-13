#!/usr/bin/python3
"""Dedicated root-owned SEC-06C browser systemd manager.

Uses the SAME verified cgroup-v2/UID-drop/hostile-ancestor primitives as the
finite Git, probe and scanner manager. Only a fixed Chromium+Node+Playwright
pair enters one root-owned leaf. The root process forwards bounded, opaque
JSONL bytes and can never execute a command chosen by the model/webpage.
"""
import argparse
import hashlib
import os
import pathlib
import pwd
import re
import runpy
import select
import stat
import subprocess
import sys
import time

KERNEL=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
MEDIATOR=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-browser-worker")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/browser-receipts")
APPROVAL=pathlib.Path("/run/lu-sec06c")
CGROUP=pathlib.Path("/sys/fs/cgroup")
HEX32=re.compile(r"^[a-f0-9]{32}$")
HEX64=re.compile(r"^[a-f0-9]{64}$")
ID=re.compile(r"^[A-Za-z0-9_-]{1,128}$")
def refuse(s):raise RuntimeError("SEC06C_BROWSER_KERNEL_DENIED: "+s)
def original_root_image(p):
    path=pathlib.Path(p)
    if not path.is_absolute() or path.is_symlink() or path.resolve(strict=True)!=path:
        refuse("untrusted first-party installed source")
    st=path.stat()
    if not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        refuse("first-party root source mutable")
    for folder in (path.parent,path.parent.parent):
        st=folder.stat()
        if st.st_uid!=0 or st.st_mode&0o022:refuse("first-party root directory mutable")
    return path
def original_image_hash(path,sha,kind):
    if not HEX64.fullmatch(sha):refuse("browser "+kind+" SHA missing")
    image=original_root_image(path)
    names={"chromium":("chrome","chromium"),"node":("node",),
           "script":("lu_browser_worker.mjs",)}
    if image.name not in names[kind]:refuse("browser executable type changed")
    h=hashlib.sha256()
    with image.open("rb") as src:
        for part in iter(lambda:src.read(65536),b""):h.update(part)
    if h.hexdigest()!=sha:refuse("reviewed "+kind+" bytes changed")
    return image
def origin_allowed(s):
    if not re.fullmatch(r"https://[a-z0-9.-]{3,253}",s):return False
    host=s[8:]
    return ("." in host and not host.startswith(".") and not host.endswith(".")
        and all(0<len(x)<=63 and x[0].isalnum() and x[-1].isalnum()
                for x in host.split("."))
        and host.rsplit(".",1)[-1].isalpha()
        and not host.endswith((".local",".internal",".localhost")))
def owned_dir(path):
    path.mkdir(mode=0o755,parents=True,exist_ok=True)
    st=path.lstat()
    if not stat.S_ISDIR(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        refuse("root-owned approval/receipt directory changed")
def fixed_browser_contract(a):
    if (a.domain!="browser" or a.op!="session" or not HEX32.fullmatch(a.nonce)
        or not ID.fullmatch(a.call) or
        not re.fullmatch(r"c-[a-f0-9]{32}",a.chat) or
        not re.fullmatch(r"g-[a-f0-9]{32}",a.turn)):
        refuse("browser original non-replayable human turn missing")
    if a.uid<=0 or pwd.getpwuid(a.uid).pw_uid!=a.uid:
        refuse("browser target must be original nonroot UID")
    if (a.mode not in ("automatic","always-visible")
        or not re.fullmatch(r":[0-9]{1,3}(?:\.[0-9]{1,2})?",a.display or "")
        or not 1<=len(a.origin)<=8 or len(set(a.origin))!=len(a.origin)
        or not all(map(origin_allowed,a.origin))):
        refuse("finite read-only browser origin/visibility contract invalid")
    profile=pathlib.Path(a.profile)
    if (not profile.is_absolute() or profile.is_symlink()
        or profile.resolve(strict=True)!=profile or not profile.is_dir()
        or profile.stat().st_uid!=a.uid or profile.stat().st_mode&0o022):
        refuse("browser profile not original user-owned canonical directory")
    if (not HEX64.fullmatch(a.policy_sha) or
        a.policy_sha!=hashlib.sha256(("\0".join(
            [str(profile),a.mode]+a.origin)+"\0").encode()).hexdigest()):
        refuse("original browser permission scope digest invalid")
    if a.xauthority:
        xauth=pathlib.Path(a.xauthority)
        if (not xauth.is_absolute() or xauth.is_symlink()
            or xauth.resolve(strict=True)!=xauth or not xauth.is_file()
            or xauth.stat().st_uid!=a.uid):
            refuse("X11 user cookie not independently owner-verified")
    chrom=original_image_hash(a.chromium,a.chromium_sha,"chromium")
    node=original_image_hash(a.node,a.node_sha,"node")
    script=original_image_hash(a.script,a.script_sha,"script")
    return [str(PYTHON),str(MEDIATOR),"--nonce",a.nonce,
        "--chromium",str(chrom),"--chromium-sha",a.chromium_sha,
        "--node",str(node),"--node-sha",a.node_sha,
        "--script",str(script),"--script-sha",a.script_sha,
        "--profile",str(profile),"--policy-sha",a.policy_sha,
        "--mode",a.mode,"--display",a.display,
        *([] if not a.xauthority else ["--xauthority",a.xauthority]),
        *["--origin="+origin for origin in a.origin]]
def first_line(fd,deadline):
    out=bytearray()
    while time.monotonic()<deadline:
        ready,_,_=select.select([fd],[],[],.04)
        if not ready:continue
        chunk=os.read(fd,1)
        if not chunk:refuse("mediator exited before browser admission")
        out.extend(chunk)
        if len(out)>256:refuse("original browser readiness frame oversized")
        if chunk==b"\n":return bytes(out)
    refuse("timed out waiting for actual jointly owned Chromium and Node")
def parse_pair(frame,nonce,leaf):
    try:
        label,identity,chromium,node,port=frame.decode("ascii").rstrip("\n").split("\t")
        if label!="LU_SEC06C_BROWSER_READY_V1" or identity!=nonce:
            refuse("original browser worker nonce/handshake mismatch")
        cpid=int(chromium);npid=int(node);nport=int(port)
        if cpid<=1 or npid<=1 or cpid==npid or not 1<=nport<=65535:
            refuse("invalid independently launched browser pair")
        expected="0::/"+str(leaf.relative_to(CGROUP))
        for pid in (cpid,npid):
            if pathlib.Path(f"/proc/{pid}/cgroup").read_text().strip()!=expected:
                refuse("Chromium and Node have different kernel owner")
        return cpid,npid
    except (ValueError,UnicodeError,OSError) as exc:
        refuse("original browser PID not verifiable: "+type(exc).__name__)
def emit(fd,bytes_):
    data=memoryview(bytes_)
    while data:data=data[os.write(fd,data):]
def relay(stdin_fd,stdout_fd,limit,pending,destination):
    raw=os.read(stdin_fd,65536)
    if not raw:return False
    pending.extend(raw)
    while b"\n" in pending:
        frame,_,rest=pending.partition(b"\n")
        if len(frame)>limit:refuse("browser protocol frame overrun")
        emit(destination,frame+b"\n")
        pending[:]=rest
    if len(pending)>limit:refuse("unterminated browser protocol frame")
    return True
def main_run(a):
    if os.geteuid()!=0:refuse("only root-owned SEC06C service may manage kernel")
    for f in (pathlib.Path(__file__).resolve(),KERNEL,MEDIATOR,PYTHON):
        original_root_image(f)
    # Reuse the original security boundary; this is not a competing cgroup
    # implementation or alternate same-UID guardian.
    k=runpy.run_path(str(KERNEL),run_name="lu_sec06c_browser_kernel_primitives")
    if (not k["process_capability_bits"]("CapEff")&(1<<7)
        or not k["process_capability_bits"]("CapPrm")&(1<<7)
        or not k["process_capability_bits"]("CapEff")&(1<<6)):
        refuse("root manager lacks genuine CAP_SETUID/CAP_SETGID")
    argv=fixed_browser_contract(a)
    parent,unit=k["first_party_unit"](a.nonce,"browser")
    owned_dir(APPROVAL);owned_dir(RECEIPTS)
    receipt=RECEIPTS/(a.nonce+".receipt")
    go=APPROVAL/(a.nonce+".go")
    if receipt.exists() or go.exists():refuse("original browser nonce replay")
    leaf=parent/("lu-sec06c-leaf-"+a.nonce)
    leaf.mkdir(mode=0o755)
    if leaf.stat().st_uid!=0 or leaf.stat().st_mode&0o022:
        refuse("worker leaf not independently root-owned")
    events=(leaf/"cgroup.events").open("r",encoding="ascii")
    if not k["kernel_empty"](events) or not (leaf/"cgroup.kill").is_file():
        refuse("original browser leaf not initially kernel-empty")
    user=pwd.getpwuid(a.uid)
    child=None
    cleanup=False
    status=93
    try:
        attack=subprocess.Popen([
            str(PYTHON),str(KERNEL),"--attack-leaf","--nonce",a.nonce
        ],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=None,
        preexec_fn=lambda:k["preexec_leaf"](leaf,a.uid,user.pw_gid,readonly=False))
        if attack.wait(timeout=10)!=0 or not k["wait_kernel_zero"](events,5):
            refuse("SEC06C_BROWSER_ORIGINAL_ANCESTOR_MIGRATION_NOT_DENIED")
        child=subprocess.Popen(argv,stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,stderr=None,cwd="/",
            env={"PATH":"/usr/bin:/bin","LC_ALL":"C","HOME":user.pw_dir},
            preexec_fn=lambda:k["preexec_leaf"](leaf,a.uid,user.pw_gid),
            close_fds=True)
        expected="0::/"+str(leaf.relative_to(CGROUP))
        if pathlib.Path(f"/proc/{child.pid}/cgroup").read_text().strip()!=expected:
            refuse("reviewed browser mediator missing pre-exec kernel admission")
        mediator_pidfd=os.pidfd_open(child.pid)
        try:
            k["original_root_file"](go,"LU_SEC06C_GO_V1\n"+a.nonce+"\n")
            frame=first_line(child.stdout.fileno(),time.monotonic()+35)
            cpid,npid=parse_pair(frame,a.nonce,leaf)
            cfd=os.pidfd_open(cpid)
            nfd=os.pidfd_open(npid)
            try:
                emit(sys.stdout.fileno(),frame)
                incoming=bytearray();outgoing=bytearray()
                deadline=time.monotonic()+3700
                while time.monotonic()<deadline and child.poll() is None:
                    for pid in (cpid,npid):
                        if pathlib.Path(f"/proc/{pid}/cgroup").read_text().strip()!=expected:
                            refuse("original browser process escaped kernel leaf")
                    ready,_,_=select.select(
                        [sys.stdin.buffer,child.stdout],[],[],.035)
                    if sys.stdin.buffer in ready:
                        if not relay(sys.stdin.fileno(),None,8192,
                                     incoming,child.stdin.fileno()):
                            status=0 # Host explicitly closed its lifetime pipe.
                            break
                    if child.stdout in ready:
                        if not relay(child.stdout.fileno(),None,3_000_000,
                                     outgoing,sys.stdout.fileno()):
                            break
            finally:
                os.close(cfd);os.close(nfd)
        finally:
            os.close(mediator_pidfd)
    finally:
        # Never produce a receipt from process leader exit or PID-group kill.
        # Even setsid descendants are inside this root-owned kernel cgroup.
        try:
            (leaf/"cgroup.kill").write_text("1",encoding="ascii")
            if not k["wait_kernel_zero"](events,15):
                refuse("browser kernel populated-zero cleanup UNVERIFIED")
            if child is not None:child.wait(timeout=10)
            leaf.rmdir()
            cleanup=True
        finally:
            if go.exists():go.unlink()
            events.close()
    if not cleanup:refuse("browser kernel release did not finish")
    k["original_root_file"](receipt,"\n".join([
        "LU_SEC06C_OWNED_BROWSER_V1","NONCE="+a.nonce,
        "CHAT="+a.chat,"TURN="+a.turn,"CALL="+a.call,
        "CHROMIUM_SHA256="+a.chromium_sha,
        "NODE_SHA256="+a.node_sha,"SCRIPT_SHA256="+a.script_sha,
        "POLICY_SHA256="+a.policy_sha,"UNIT="+unit,
        "ORIGINAL_UID="+str(a.uid),"KERNEL_EMPTY=1",
        "EFFECT="+("STOPPED" if status==0 else "INTERRUPTED"),""]))
    return 0 if status==0 else 73
def main():
    p=argparse.ArgumentParser(allow_abbrev=False)
    for name in ("domain","op","nonce","chat","turn","call","uid",
                 "chromium","chromium_sha","node","node_sha",
                 "script","script_sha","profile","policy_sha",
                 "mode","display"):
        p.add_argument("--"+name.replace("_","-"),required=True)
    p.add_argument("--xauthority",default="")
    p.add_argument("--origin",action="append",default=[])
    try:
        a=p.parse_args();a.uid=int(a.uid)
        return main_run(a)
    except Exception as e:
        print("SEC06C_BROWSER_MANAGER_BLOCKED: "+str(e),file=sys.stderr)
        return 91
if __name__=="__main__":sys.exit(main())
