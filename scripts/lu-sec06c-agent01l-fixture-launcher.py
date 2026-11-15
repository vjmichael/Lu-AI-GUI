#!/usr/bin/python3
"""Finite, root-owned sudo entrypoint for AGENT-01L original crash fixture only.

No arbitrary binary, command, cgroup path, executable, signal, PID kill,
filesystem destination or model input. The separately root-owned systemd
manager performs kernel admission and descendant kill after original host
SIGKILL. Installed only on an administrator-authorized native Linux runner.
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
import secrets

RUN=pathlib.Path("/usr/bin/systemd-run")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
MANAGER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
FIXTURE_MANAGER=pathlib.Path("/usr/libexec/lu/lu-sec06c-agent01l-fixture-manager")
HOST_MANIFEST=pathlib.Path("/usr/libexec/lu/lu-agent01l-fixture-host.sha256")
ID=re.compile(r"^[0-9]{1,10}$")


def deny(message):
    raise RuntimeError("AGENT01L_SEC06C_FIXED_LAUNCHER_DENIED: "+message)


def root_file(path):
    path=pathlib.Path(path)
    if not path.is_absolute() or path.is_symlink():
        deny("untrusted first-party executable path")
    p=path.stat()
    if not stat.S_ISREG(p.st_mode) or p.st_uid!=0 or p.st_mode&0o022:
        deny("original installed executable not immutable/root-owned")
    for root in (path.parent,path.parent.parent):
        m=root.stat()
        if m.st_uid!=0 or m.st_mode&0o022:
            deny("original executable directory writable by nonroot")
    return path


def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    ap.add_argument("--host-pid",required=True)
    ap.add_argument("--profile",required=True)
    args=ap.parse_args()
    if os.geteuid()!=0 or "SUDO_UID" not in os.environ:
        deny("must be an independently sudo-authorized fixed test launcher")
    if not ID.fullmatch(args.host_pid):
        deny("untrusted original host PID syntax")
    uid=int(os.environ["SUDO_UID"])
    if uid<=0 or pwd.getpwuid(uid).pw_uid!=uid:
        deny("original test runner must be nonroot")
    pid=int(args.host_pid)
    status=pathlib.Path(f"/proc/{pid}/status").read_text(encoding="ascii")
    uid_rows=[x.split() for x in status.splitlines() if x.startswith("Uid:")]
    if len(uid_rows)!=1 or len(uid_rows[0])!=5 or int(uid_rows[0][1])!=uid:
        deny("original test parent does not belong to original caller")
    exe=pathlib.Path(f"/proc/{pid}/exe").resolve(strict=True)
    if exe.name!="lu-e1-p08-linux-cgroup-crash-parent":
        deny("not the first-party original test host")
    root_file(HOST_MANIFEST)
    pinned=HOST_MANIFEST.read_text(encoding="ascii")
    if len(pinned)!=65 or pinned[-1]!="\n" or not re.fullmatch(
            r"[0-9a-f]{64}",pinned[:-1]):
        deny("root-owned original fixture host digest invalid")
    # Read the kernel's LIVE /proc/<pid>/exe inode, not a mutable path
    # the runner could replace between process launch and a pathname hash.
    h=hashlib.sha256()
    with pathlib.Path(f"/proc/{pid}/exe").open("rb") as live:
        for chunk in iter(lambda:live.read(65536),b""):
            h.update(chunk)
    if h.hexdigest()!=pinned[:-1]:
        deny("original executable does not match reviewed root-owned host SHA")
    profile=pathlib.Path(args.profile)
    if not profile.is_absolute() or profile.is_symlink() or profile.resolve(strict=True)!=profile:
        deny("untrusted original fixture profile")
    if not profile.name.startswith("lu-agent01l-cgroup-"):
        deny("not the original test profile")
    meta=profile.lstat()
    if (not stat.S_ISDIR(meta.st_mode) or meta.st_uid!=uid
            or meta.st_mode&0o077):
        deny("fixture profile not exclusively owned by original test runner")
    for file in (pathlib.Path(__file__).resolve(),RUN,PYTHON,MANAGER,FIXTURE_MANAGER):
        root_file(file)
    nonce=secrets.token_hex(16)
    unit="lu-sec06c-fixture-"+nonce+".service"
    cmd=[
        str(RUN),"--system","--quiet","--wait","--pipe",
        "--no-ask-password","--service-type=exec",f"--unit={unit}",
        "--property=User=root","--property=Delegate=yes",
        "--property=KillMode=control-group",
        "--property=PrivateNetwork=yes",
        "--property=NoNewPrivileges=yes",
        "--property=CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_KILL",
        "--property=AmbientCapabilities=CAP_SETUID",
        "--property=RestrictSUIDSGID=yes",
        "--property=TasksMax=32","--property=MemoryMax=536870912",
        "--property=RuntimeMaxSec=65",
        str(PYTHON),str(FIXTURE_MANAGER),
        "--nonce",nonce,"--uid",str(uid),"--host-pid",str(pid),
        "--profile",str(profile),
    ]
    proc=subprocess.run(cmd,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,cwd="/",timeout=75,
                        check=False,env={"PATH":"/usr/bin:/bin","LC_ALL":"C"})
    if proc.returncode!=0:
        deny("original protected root-owned systemd fixture failed")
    return 0


if __name__=="__main__":
    try:sys.exit(main())
    except Exception as exc:
        print("AGENT01L_SEC06C_FIXED_LAUNCHER_BLOCKED: "+str(exc),
              file=sys.stderr)
        sys.exit(91)
