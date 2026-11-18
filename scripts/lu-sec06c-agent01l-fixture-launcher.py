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
import select
import time

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



# Bounded provenance from the exact nonce-named ROOT service. This diagnostic
# cannot grant custody: only the original untouched kernel tests can pass it.
MAX_SERVICE_STDERR=4096
DIAGNOSTIC_DIR=pathlib.Path("/var/lib/lu-sec06c/fixture-receipts")


def root_diagnostic(nonce,unit,returncode,raw,truncated,timed_out):
    if DIAGNOSTIC_DIR.is_symlink():
        deny("root-owned diagnostic directory is a symlink")
    meta=DIAGNOSTIC_DIR.stat()
    if (not stat.S_ISDIR(meta.st_mode) or meta.st_uid!=0
            or meta.st_mode&0o022):
        deny("root-owned diagnostic directory not trusted")
    safe="".join(chr(c) if c in (9,10,13) or 32<=c<=126 else "?"
                 for c in raw[:MAX_SERVICE_STDERR])
    body=("AGENT01L_ROOT_SERVICE_DIAGNOSTIC_V1\n"
          f"UNIT={unit}\nEXIT={returncode}\n"
          f"STDERR_TRUNCATED={int(truncated)}\n"
          f"TIMED_OUT={int(timed_out)}\n"
          "STDERR_BEGIN\n"+safe+"\nSTDERR_END\n")
    path=DIAGNOSTIC_DIR/(nonce+".diagnostic")
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,"w",encoding="ascii") as out:
        out.write(body)
        out.flush()
        os.fsync(out.fileno())
    folder=os.open(DIAGNOSTIC_DIR,os.O_RDONLY|os.O_DIRECTORY)
    try:
        os.fsync(folder)
    finally:
        os.close(folder)


def bounded_service_result(cmd):
    # Always drain the ROOT service stderr to avoid deadlock, but keep at most
    # 4096 bytes in memory; never forward arbitrary service output to a user.
    proc=subprocess.Popen(cmd,stdin=subprocess.DEVNULL,
                          stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,
                          cwd="/",env={"PATH":"/usr/bin:/bin","LC_ALL":"C"})
    observed=bytearray()
    truncated=False
    timed_out=False
    poller=select.poll()
    poller.register(proc.stderr.fileno(),select.POLLIN|select.POLLHUP|select.POLLERR)
    deadline=time.monotonic()+75
    while True:
        left=deadline-time.monotonic()
        if left<=0:
            timed_out=True
            proc.kill()
            break
        ready=poller.poll(min(250,max(1,int(left*1000))))
        if not ready:
            continue
        chunk=os.read(proc.stderr.fileno(),4096)
        if not chunk:
            break
        room=MAX_SERVICE_STDERR-len(observed)
        observed.extend(chunk[:room])
        if len(chunk)>room:
            truncated=True
    try:
        returncode=proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        returncode=proc.wait(timeout=5)
        timed_out=True
    proc.stderr.close()
    return (124 if timed_out else returncode),bytes(observed),truncated,timed_out



def kernel_starttime(pid):
    # /proc/PID/stat field 22; unlike /proc/PID/exe this does not require
    # cross-user ptrace privileges. Recheck before root service admission.
    raw=pathlib.Path(f"/proc/{pid}/stat").read_text(encoding="ascii")
    if ") " not in raw:
        deny("original kernel process stat invalid")
    fields=raw.rsplit(")",1)[1].split()
    if len(fields)<20 or not re.fullmatch(r"[0-9]{1,30}",fields[19]):
        deny("original kernel PID starttime unavailable")
    return fields[19]


def root_admission(nonce,pid,uid,starttime,digest):
    # The privileged launcher verified live /proc/PID/exe against a
    # ROOT-owned manifest. Seal only that exact PID + starttime + digest
    # so the restricted service need not receive CAP_SYS_PTRACE.
    if DIAGNOSTIC_DIR.is_symlink():
        deny("root-owned admission directory is a symlink")
    info=DIAGNOSTIC_DIR.stat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid!=0
            or info.st_mode&0o022):
        deny("original admission directory must be root-owned and immutable")
    body=("AGENT01L_ROOT_VERIFIED_HOST_V1\n"
          f"NONCE={nonce}\nHOST_PID={pid}\nUID={uid}\n"
          f"STARTTIME={starttime}\nEXEC_SHA256={digest}\n")
    target=DIAGNOSTIC_DIR/(nonce+".admission")
    fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,"w",encoding="ascii") as out:
        out.write(body)
        out.flush()
        os.fsync(out.fileno())
    fd=os.open(DIAGNOSTIC_DIR,os.O_RDONLY|os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


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
    initial_start=kernel_starttime(pid)
    h=hashlib.sha256()
    with pathlib.Path(f"/proc/{pid}/exe").open("rb") as live:
        for chunk in iter(lambda:live.read(65536),b""):
            h.update(chunk)
    if h.hexdigest()!=pinned[:-1]:
        deny("original executable does not match reviewed root-owned host SHA")
    if kernel_starttime(pid)!=initial_start:
        deny("original kernel PID identity changed across executable verification")
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
    root_admission(nonce,pid,uid,initial_start,pinned[:-1])
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
    returncode,stderr,truncated,timed_out=bounded_service_result(cmd)
    if returncode!=0:
        root_diagnostic(nonce,unit,returncode,stderr,truncated,timed_out)
        deny("original protected root-owned systemd fixture failed; "
             "root-owned diagnostic is sealed to exact unit "+unit)
    return 0


if __name__=="__main__":
    try:sys.exit(main())
    except Exception as exc:
        print("AGENT01L_SEC06C_FIXED_LAUNCHER_BLOCKED: "+str(exc),
              file=sys.stderr)
        sys.exit(91)
