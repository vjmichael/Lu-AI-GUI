#!/usr/bin/python3
"""SEC-06C dedicated, one-use browser-session launcher; never a shell or model executor.

Only root-owned installed Chromium, Node and reviewed Lu Playwright script are
accepted. All three SHA-256 digests, the human-turn IDs and origin policy
are rechecked independently by the OS-owned kernel manager. Browsing is the
only network-enabled SEC-06C domain; no unreviewed executable is selected.
"""
import argparse
import hashlib
import os
import pathlib
import pwd
import re
import stat
import subprocess
import sys

MANAGER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-browser-manager")
KERNEL=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
MEDIATOR=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-browser-worker")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
SYSTEMD=pathlib.Path("/usr/bin/systemd-run")
RUN=pathlib.Path("/run/lu-sec06c")
RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/browser-receipts")
HEX32=re.compile(r"^[a-f0-9]{32}$")
HEX64=re.compile(r"^[a-f0-9]{64}$")
ID=re.compile(r"^[A-Za-z0-9_-]{1,128}$")
DISPLAY=re.compile(r"^:[0-9]{1,3}(?:\.[0-9]{1,2})?$")
def deny(reason):raise RuntimeError("SEC06C_BROWSER_BROKER_DENIED: "+reason)
def root_file(raw):
    p=pathlib.Path(raw)
    if not p.is_absolute() or p.is_symlink():deny("installed image symlink")
    m=p.stat()
    if not stat.S_ISREG(m.st_mode) or m.st_uid!=0 or m.st_mode&0o022:
        deny("installed executable not root-owned")
    for parent in (p.parent,p.parent.parent):
        m=parent.stat()
        if m.st_uid!=0 or m.st_mode&0o022:deny("installed directory mutable")
    return p
def digest(raw,expected):
    if not HEX64.fullmatch(expected):deny("missing reviewed digest")
    p=root_file(raw)
    if p.resolve(strict=True)!=p:deny("noncanonical installed executable")
    h=hashlib.sha256()
    with p.open("rb") as f:
        for data in iter(lambda:f.read(65536),b""):h.update(data)
    if h.hexdigest()!=expected:deny("reviewed executable bytes changed")
    return p
def origin(s):
    if len(s)>256 or not re.fullmatch(r"https://[a-z0-9.-]+",s):
        return False
    name=s[8:]
    return (len(name)<=253 and "." in name and not name.startswith(".")
        and not name.endswith(".") and all(
            0<len(p)<=63 and p[0].isalnum() and p[-1].isalnum()
            for p in name.split("."))
        and name.rsplit(".",1)[-1].isalpha()
        and not name.endswith((".local",".internal",".localhost")))
def policy(profile,mode,origins):
    return hashlib.sha256(("\0".join([profile,mode]+origins)+"\0").encode()).hexdigest()
def directory(path):
    p=pathlib.Path(path)
    # A fresh CI/installed machine need not already have the original
    # SEC-06C runtime directory. Provision only this fixed root-owned path;
    # never create writable ancestors or delegate cgroup ownership.
    p.mkdir(mode=0o755,parents=True,exist_ok=True)
    for checked in (p,p.parent,p.parent.parent):
        m=checked.lstat() # No symlink in any root-owned admission ancestor.
        if (not stat.S_ISDIR(m.st_mode) or m.st_uid!=0
            or m.st_mode&0o022):
            deny("root-owned nonce or receipt directory/ancestor mutable")
def run(a):
    if os.geteuid()!=0 or "SUDO_UID" not in os.environ:
        deny("requires exact fixed sudo policy")
    uid=int(os.environ["SUDO_UID"])
    if uid<=0 or pwd.getpwuid(uid).pw_uid!=uid:deny("missing original nonroot user")
    if (not HEX32.fullmatch(a.nonce) or not all(ID.fullmatch(x)
        for x in (a.chat,a.turn,a.call)) or not a.chat.startswith("c-")
        or not a.turn.startswith("g-") or len(a.chat)!=34 or len(a.turn)!=34
        or not HEX32.fullmatch(a.chat[2:]) or not HEX32.fullmatch(a.turn[2:])):
        deny("missing original authenticated turn identity")
    if a.mode not in ("automatic","always-visible") or not DISPLAY.fullmatch(a.display):
        deny("unreviewed visibility or graphical display")
    if not 1<=len(a.origin)<=8 or len(set(a.origin))!=len(a.origin) or not all(map(origin,a.origin)):
        deny("invalid finite read-only browser origins")
    for image in (pathlib.Path(__file__).resolve(),MANAGER,KERNEL,MEDIATOR,PYTHON,SYSTEMD):
        root_file(image)
    for binary,sha in ((a.chromium,a.chromium_sha),(a.node,a.node_sha),(a.script,a.script_sha)):
        digest(binary,sha)
    if pathlib.Path(a.chromium).name not in ("chrome","chromium") or pathlib.Path(a.node).name!="node" or pathlib.Path(a.script).name!="lu_browser_worker.mjs":
        deny("reviewed browser executable type mismatch")
    profile=pathlib.Path(a.profile)
    if (not profile.is_absolute() or profile.is_symlink() or
        profile.resolve(strict=True)!=profile or not profile.is_dir() or
        profile.stat().st_uid!=uid or profile.stat().st_mode&0o022):
        deny("untrusted original user browser profile")
    if a.policy_sha!=policy(str(profile),a.mode,a.origin):
        deny("exact browser grant policy hash mismatch")
    if not HEX64.fullmatch(a.policy_sha):deny("invalid policy digest")
    # No original user-provided app-supplied XAUTHORITY is promoted to an
    # arbitrary file path. An optional checked user-owned X11 cookie is read
    # only by the nonroot mediator.
    if a.xauthority:
        xauth=pathlib.Path(a.xauthority)
        if (not xauth.is_absolute() or xauth.is_symlink() or
            xauth.resolve(strict=True)!=xauth or not xauth.is_file()
            or xauth.stat().st_uid!=uid):
            deny("untrusted X11 user cookie")
    directory(RUN);directory(RECEIPTS)
    if (RUN/(a.nonce+".go")).exists() or (RECEIPTS/(a.nonce+".receipt")).exists():
        deny("one-use browser nonce already consumed")
    unit="lu-sec06c-browser-"+a.nonce+".service"
    cmd=[str(SYSTEMD),"--system","--quiet","--wait","--pipe",
        "--no-ask-password","--service-type=exec","--unit="+unit,
        "--property=User=root","--property=Delegate=yes",
        "--property=KillMode=control-group","--property=NoNewPrivileges=yes",
        "--property=RestrictSUIDSGID=yes",
        "--property=CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_KILL",
        "--property=AmbientCapabilities=CAP_SETUID",
        "--property=TasksMax=128","--property=MemoryMax=4294967296",
        "--property=RuntimeMaxSec=3900",
        # Unlike fixed offline scanners, this reviewed NetworkRead browser
        # requires networking; origins remain confined by the Playwright gate.
        str(PYTHON),str(MANAGER),
        "--domain","browser","--nonce",a.nonce,"--chat",a.chat,
        "--turn",a.turn,"--call",a.call,"--uid",str(uid),"--op","session",
        "--chromium",a.chromium,"--chromium-sha",a.chromium_sha,
        "--node",a.node,"--node-sha",a.node_sha,
        "--script",a.script,"--script-sha",a.script_sha,
        "--profile",str(profile),"--policy-sha",a.policy_sha,
        "--mode",a.mode,"--display",a.display]
    if a.xauthority:cmd+=["--xauthority",a.xauthority]
    for x in a.origin:cmd.append("--origin="+x)
    # Forward only the original stdin/stdout stream. The privileged launcher
    # never parses page JSON or browser replies.
    return subprocess.call(cmd,cwd="/",timeout=3960)
def main():
    p=argparse.ArgumentParser(allow_abbrev=False)
    for name in ("nonce","chat","turn","call","chromium","chromium_sha",
                 "node","node_sha","script","script_sha","profile",
                 "policy_sha","mode","display"):
        p.add_argument("--"+name.replace("_","-"),required=True)
    p.add_argument("--xauthority",default="")
    p.add_argument("--origin",action="append",default=[])
    try:return run(p.parse_args())
    except Exception as e:
        print("SEC06C_BROWSER_BROKER_BLOCKED: "+str(e),file=sys.stderr)
        return 91
if __name__=="__main__":sys.exit(main())
