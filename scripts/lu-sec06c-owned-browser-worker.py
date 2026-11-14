#!/usr/bin/python3
"""Nonroot SEC-06C browser mediator: one headed Chromium and one Node worker.

Both are descendants of the SAME root-owned systemd leaf. This nonprivileged
script merely pins their fixed arguments, validates CDP loopback and forwards
bounded JSONL frames. Human visibility grants remain solely in the native host.
"""
import argparse
import hashlib
import json
import os
import pathlib
import re
import select
import stat
import subprocess
import sys
import time
import tempfile

MAX_REQUEST=8192
MAX_REPLY=3_000_000
HEX32=re.compile(r"^[0-9a-f]{32}$")
HEX64=re.compile(r"^[0-9a-f]{64}$")
def reject(s):raise RuntimeError("SEC06C_BROWSER_MEDIATOR_DENIED: "+s)
def owned_image(path,digest):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink() or p.resolve(strict=True)!=p:
        reject("untrusted executable pathname")
    st=p.stat()
    if st.st_uid!=0 or st.st_mode&0o022 or not stat.S_ISREG(st.st_mode):
        reject("mutable executable binary")
    with p.open("rb") as f:
        h=hashlib.sha256()
        for block in iter(lambda:f.read(65536),b""):h.update(block)
    if h.hexdigest()!=digest:reject("reviewed executable digest changed")
def validate_original_user_xauthority(raw):
    """Only the unprivileged original UID may inspect its X11 cookie.

    Refuse links, directory substitution, permissive/foreign credentials and
    unreadable cookies; the privileged manager never opens this user path.
    """
    xauth=pathlib.Path(raw)
    if not xauth.is_absolute() or xauth.is_symlink():
        reject("untrusted nonroot X11 cookie pathname")
    if xauth.resolve(strict=True)!=xauth:
        reject("noncanonical X11 cookie path")
    meta=xauth.lstat()
    if (not stat.S_ISREG(meta.st_mode) or meta.st_uid!=os.geteuid()
        or meta.st_mode&0o077 or meta.st_nlink!=1 or
        not 0<meta.st_size<=65536):
        reject("original user X11 cookie owner, mode or size invalid")
    return str(xauth)

def require_go(nonce):
    p=pathlib.Path("/run/lu-sec06c")/(nonce+".go")
    until=time.monotonic()+20
    while time.monotonic()<until:
        try:st=p.lstat()
        except FileNotFoundError:
            time.sleep(.02);continue
        if (not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022
            or st.st_nlink!=1 or p.read_text(encoding="ascii")!="LU_SEC06C_GO_V1\n"+nonce+"\n"):
            reject("missing original systemd one-use start authorization")
        return
    reject("root manager never released exact browser session")
def loopback_port(path):
    try:st=path.lstat()
    except FileNotFoundError:return None
    if not stat.S_ISREG(st.st_mode) or st.st_size>512 or st.st_size==0:
        reject("untrusted DevToolsActivePort")
    rows=path.read_text(encoding="ascii").splitlines()
    if len(rows)!=2 or not re.fullmatch(r"/devtools/browser/[A-Za-z0-9_-]{1,200}",rows[1]):
        reject("invalid browser-only websocket endpoint")
    port=int(rows[0])
    if not 1<=port<=65535:reject("non-loopback DevTools endpoint")
    return port
def forward(src,dst,limit,partial):
    chunk=os.read(src,65536)
    if not chunk:return False
    # Frames may be chunked across reads. Neither model nor webpage may
    # force unbounded memory, turn a JSONL field into a shell command, or
    # pass a multi-megabyte request into a more privileged parser.
    partial.extend(chunk)
    while b"\n" in partial:
        frame,_,remain=partial.partition(b"\n")
        if len(frame)>limit:reject("bounded original browser frame overflow")
        if dst is not None:
            view=memoryview(frame+b"\n")
            while view:
                n=os.write(dst,view)
                view=view[n:]
        partial[:]=remain
    if len(partial)>limit:reject("unbounded browser frame")
    return True
def chrome_pre_admission_failure(chrome,log,profile,xauthority):
    """Only post-exit, bounded original Chromium stderr; never page output.

    Chromium's exit status alone cannot distinguish X11, executable, linker,
    sandbox or namespace admission. The browser protocol stays opaque; a
    crash at this stage NEVER releases a valid browser kernel receipt.
    """
    log.flush()
    log.seek(0)
    text=log.read(4096).decode("utf-8",errors="replace")
    for private in (str(profile),xauthority):
        if private:text=text.replace(private,"<original-user-private-path>")
    text=" ".join(x for x in text.replace("\x1b","").split() if x)
    text=text[:1800] if text else "<no-chromium-diagnostic>"
    print("SEC06C_BROWSER_CHROMIUM_PRE_ADMISSION_FAILED "
          +"exit="+str(chrome.returncode)+" stderr="+text,file=sys.stderr,flush=True)

def run(a):
    if os.geteuid()==0:reject("browser process mediator must never run as root")
    if not HEX32.fullmatch(a.nonce):reject("original manager nonce invalid")
    for image,digest in ((a.chromium,a.chromium_sha),
                         (a.node,a.node_sha),(a.script,a.script_sha)):
        if not HEX64.fullmatch(digest):reject("original binary digest invalid")
        owned_image(image,digest)
    if (a.mode not in ("automatic","always-visible") or not 1<=len(a.origin)<=8
        or len(set(a.origin))!=len(a.origin)
        or any(not re.fullmatch(r"https://[a-z0-9.-]{3,253}",o) for o in a.origin)):
        reject("browser origin policy invalid")
    profile=pathlib.Path(a.profile)
    if (not profile.is_absolute() or profile.is_symlink() or
        profile.resolve(strict=True)!=profile or not profile.is_dir()
        or profile.stat().st_uid!=os.geteuid()
        or profile.stat().st_mode&0o022):
        reject("not original nonroot profile")
    if (hashlib.sha256(("\0".join([str(profile),a.mode]+a.origin)+"\0")
        .encode()).hexdigest()!=a.policy_sha):
        reject("changed browser human-origin authority")
    env={"PATH":"/usr/bin:/bin","LC_ALL":"C","HOME":str(pathlib.Path.home()),
         "DISPLAY":a.display}
    if a.xauthority:
        # The original Linux UID, NOT the root manager, checks X11 ownership.
        env["XAUTHORITY"]=validate_original_user_xauthority(a.xauthority)
    require_go(a.nonce)
    portfile=profile/"DevToolsActivePort"
    if portfile.exists():reject("stale CDP endpoint or reused user profile")
    # An unlinked nonroot scratch file cannot block Chrome if it logs more
    # than a pipe's capacity. Read only a small sanitized prefix after a
    # proven pre-admission exit; do not weaken any sandbox flags.
    startup_log=tempfile.TemporaryFile(mode="w+b")
    try:
        chrome=subprocess.Popen([a.chromium,
             "--remote-debugging-port=0","--remote-debugging-address=127.0.0.1",
             "--no-first-run","--no-default-browser-check",
             "--disable-background-networking","--user-data-dir="+str(profile),
             "about:blank"],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
             stderr=startup_log,env=env,cwd="/",close_fds=True)
    except Exception:
        startup_log.close()
        raise
    node=None
    try:
        until=time.monotonic()+25
        port=None
        while time.monotonic()<until:
            if chrome.poll() is not None:
                chrome_pre_admission_failure(chrome,startup_log,profile,a.xauthority)
                reject("original Chromium died before admission")
            port=loopback_port(portfile)
            if port is not None:break
            time.sleep(.05)
        if port is None:
            chrome_pre_admission_failure(chrome,startup_log,profile,a.xauthority)
            reject("Chromium never established genuine loopback CDP")
        env["LU_BROWSER_HOST_CONFIG"]=json.dumps({
             "origins":a.origin,"profileDirectory":str(profile),
             "visibilityMode":a.mode,
             "cdpEndpoint":"http://127.0.0.1:"+str(port)},separators=(",",":"))
        node=subprocess.Popen([a.node,a.script],stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,
            env=env,cwd=str(pathlib.Path(a.script).parent),close_fds=True)
        if chrome.poll() is not None or node.poll() is not None:
            if chrome.poll() is not None:
                chrome_pre_admission_failure(chrome,startup_log,profile,a.xauthority)
            reject("browser pair exited before original admission")
        # The privileged root manager independently reads BOTH /proc cgroups
        # before it forwards this fixed tuple to the desktop host.
        sys.stdout.write("LU_SEC06C_BROWSER_READY_V1\t"+a.nonce+"\t"+
            str(chrome.pid)+"\t"+str(node.pid)+"\t"+str(port)+"\n")
        sys.stdout.flush()
        inp=bytearray()
        out=bytearray()
        while chrome.poll() is None and node.poll() is None:
            ready,_,_=select.select([sys.stdin.buffer,node.stdout],[],[],.1)
            if sys.stdin.buffer in ready and not forward(
                sys.stdin.fileno(),node.stdin.fileno(),MAX_REQUEST,inp):
                break
            if node.stdout in ready and not forward(
                node.stdout.fileno(),sys.stdout.fileno(),MAX_REPLY,out):
                break
        return 0 if chrome.poll() is None and node.poll() is None else 73
    finally:
        if node is not None and node.poll() is None:
            node.kill();node.wait(timeout=8)
        if chrome.poll() is None:
            chrome.kill();chrome.wait(timeout=8)
        startup_log.close()
def main():
    p=argparse.ArgumentParser(allow_abbrev=False)
    for name in ("nonce","chromium","chromium_sha","node","node_sha",
                 "script","script_sha","profile","policy_sha",
                 "mode","display"):
        p.add_argument("--"+name.replace("_","-"),required=True)
    p.add_argument("--xauthority",default="")
    p.add_argument("--origin",action="append",default=[])
    try:return run(p.parse_args())
    except Exception as e:
        print("SEC06C_BROWSER_MEDIATOR_BLOCKED: "+str(e),file=sys.stderr)
        return 93
if __name__=="__main__":sys.exit(main())
