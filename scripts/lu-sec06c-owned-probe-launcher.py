#!/usr/bin/python3
"""Dedicated SEC-06C root-owned broker for finite read-only media/metadata.

This is a DISTINCT installed sudo policy from Git. It accepts only ffprobe
WAV/MP4, pdfinfo PDF, exiftool PNG/JPEG. A root-owned first-party Python
worker runs as the calling nonroot user in a root-owned systemd cgroup.
No model-selected command, flags, worker binary, destination or arbitrary
root file read is possible. The worker re-hashes the input as nonroot and
pins an opened original FD into the reviewed subprocess.
"""
import argparse
import hashlib
import os
import pathlib
import pwd
import re
import select
import stat
import subprocess
import sys
import time

WORKER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-probe-worker")
PYTHON=pathlib.Path("/usr/bin/python3").resolve(strict=True)
RUN=pathlib.Path("/usr/bin/systemd-run")
CTL=pathlib.Path("/usr/bin/systemctl")
APPROVAL=pathlib.Path("/run/lu-sec06c")
RECEIPTS=pathlib.Path("/var/lib/lu-sec06c/probe-receipts")
MANAGER=pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
OPS={
  "ffprobe-wav": ("ffprobe",("wav",),256*1024*1024),
  "ffprobe-mp4": ("ffprobe",("mp4",),256*1024*1024),
  "pdfinfo-pdf": ("pdfinfo",("pdf",),32*1024*1024),
  "exiftool-image": ("exiftool",("png","jpg","jpeg"),32*1024*1024),
}
IDENT=re.compile(r"^[a-zA-Z0-9_-]{1,128}$")
NONCE=re.compile(r"^[a-f0-9]{32}$")
SHA=re.compile(r"^[a-f0-9]{64}$")
def deny(s):raise RuntimeError("SEC06C_PROBE_BROKER_DENIED: "+s)
def secure_file(path):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink():deny("trusted installed file missing")
    st=p.stat()
    if not stat.S_ISREG(st.st_mode) or st.st_uid!=0 or st.st_mode&0o022:
        deny("installed file not root-owned immutable")
    for parent in (p.parent,p.parent.parent):
        meta=parent.stat()
        if meta.st_uid!=0 or meta.st_mode&0o022:
            deny("installed executable parent mutable")
    return p
def secure_directory(path):
    path.mkdir(parents=True,mode=0o755,exist_ok=True)
    if path.is_symlink():deny("OS approval/receipt dir symlink")
    st=path.stat()
    if st.st_uid!=0 or not stat.S_ISDIR(st.st_mode) or st.st_mode&0o022:
        deny("OS receipt/approval directory is mutable")
def root_record(path,data):
    flags=os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW
    fd=os.open(path,flags,0o644)
    with os.fdopen(fd,"w",encoding="ascii") as f:
        f.write(data);f.flush();os.fsync(f.fileno())
    fd=os.open(path.parent,os.O_RDONLY|os.O_DIRECTORY)
    try:os.fsync(fd)
    finally:os.close(fd)
def pinned_binary(p,sha,kind):
    path=pathlib.Path(p)
    if not path.is_absolute() or path.name!=OPS[kind][0] or path.is_symlink():
        deny("not a finite approved probe binary")
    actual=path.resolve(strict=True)
    if actual!=path or actual.stat().st_uid!=0 or actual.stat().st_mode&0o022:
        deny("probe binary not immutable or root-owned")
    if not SHA.fullmatch(sha):deny("bad reviewed binary sha")
    h=hashlib.sha256()
    with path.open("rb") as f:
        for data in iter(lambda:f.read(65536),b""):h.update(data)
    if h.hexdigest()!=sha:deny("reviewed probe binary changed")
    return actual
def approved_resource(path,kind,sha):
    p=pathlib.Path(path)
    if not p.is_absolute() or p.is_symlink() or p.resolve(strict=True)!=p:
        deny("resource is not canonical original host path")
    if p.suffix.lower().lstrip(".") not in OPS[kind][1] or not SHA.fullmatch(sha):
        deny("resource type or original digest invalid")
    st=p.stat() # NEVER read model-selected input bytes as privileged root.
    if not stat.S_ISREG(st.st_mode) or not 0<st.st_size<=OPS[kind][2]:
        deny("original file size invalid")
    return p
def native_cgroup(unit):
    show=subprocess.run([str(CTL),"show",unit,
        "-p","MainPID","-p","ControlGroup","-p","ActiveState",
        "--no-pager"],capture_output=True,text=True,timeout=5)
    if show.returncode:return None
    p=dict(x.split("=",1) for x in show.stdout.splitlines() if "=" in x)
    group=p.get("ControlGroup","")
    pid=p.get("MainPID","")
    if not group.startswith("/") or not group.endswith("/"+unit) or not pid.isdigit():
        return None
    if int(pid)<=1:return None
    path=(pathlib.Path("/sys/fs/cgroup")/group.lstrip("/")).resolve(strict=True)
    if path.stat().st_uid!=0:deny("kernel operation is not owned by trusted system manager")
    membership=pathlib.Path(f"/proc/{pid}/cgroup").read_text().strip()
    if membership!="0::"+group:deny("OS unit PID/cgroup identity mismatch")
    if not (path/"cgroup.kill").is_file():deny("no genuine cgroup.kill")
    return path
def original_service_main(unit):
    """Trusted systemd fields for the exact randomly pinned operation unit."""
    status=subprocess.run([str(CTL),"show",unit,
        "-p","ActiveState","-p","SubState",
        "-p","ExecMainCode","-p","ExecMainStatus","--no-pager"],
        capture_output=True,text=True,timeout=4)
    if status.returncode:
        deny("root-owned systemd operation status cannot be observed")
    p=dict(row.split("=",1) for row in status.stdout.splitlines() if "=" in row)
    if p.get("ActiveState") not in ("active","activating"):
        deny("trusted operation unit deactivated before kernel-empty witness")
    if p.get("SubState")=="exited" and p.get("ExecMainCode")=="1":
        try:
            code=int(p["ExecMainStatus"])
        except (ValueError,KeyError):
            deny("missing original service main exit status")
        return code
    return None

def wait_populated_zero(events,timeout):
    """Never accept absence of a unit/cgroup as proof of empty descendants."""
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        if kernel_empty(events):return True
        time.sleep(.025)
    return False

def kill_exact_root_owned_subtree(group):
    """Only the kernel cgroup pinned from systemd's exact original unit."""
    f=group/"cgroup.kill"
    if not f.is_file():
        deny("original cgroup.kill disappeared")
    with f.open("w",encoding="ascii") as out:
        out.write("1")
        out.flush()

def kernel_empty(stream):
    try:
        stream.seek(0)
        return [x for x in stream.read().splitlines() if x.startswith("populated ")]==["populated 0"]
    except OSError:return False
def run(args):
    if os.geteuid()!=0 or "SUDO_UID" not in os.environ:
        deny("finite probe broker needs original sudo admission")
    uid=int(os.environ["SUDO_UID"])
    if uid<=0 or pwd.getpwuid(uid).pw_uid!=uid:
        deny("finite media worker must run as original nonroot UID")
    if any(not IDENT.fullmatch(x) for x in [args.chat,args.turn,args.call]):
        deny("original human resource identity invalid")
    if not NONCE.fullmatch(args.nonce) or args.kind not in OPS:
        deny("not one of four finite reviewed probe kinds")
    for f in (pathlib.Path(__file__).resolve(),WORKER,PYTHON,MANAGER,RUN,CTL):
        secure_file(f)
    binary=pinned_binary(args.binary,args.binary_sha,args.kind)
    resource=approved_resource(args.file,args.kind,args.resource_sha)
    secure_directory(APPROVAL);secure_directory(RECEIPTS)
    receipt=RECEIPTS/(args.nonce+".receipt")
    go=APPROVAL/(args.nonce+".go")
    if receipt.exists() or go.exists():
        deny("original probe nonce already consumed")
    unit=f"lu-sec06c-probe-{args.nonce}.service"
    # Unlike Git, this fixed broker has its own finite media-kind/resource
    # arguments, SHA256 and root receipt type. The only systemd ExecStart
    # is the SAME trusted root-owned kernel manager, which permits only its
    # two separately reviewed first-party worker domains, never user CLI.
    command=[
        str(RUN),"--system","--quiet","--wait","--pipe",
        "--no-ask-password","--service-type=exec",f"--unit={unit}",
        "--property=User=root","--property=Delegate=yes",
        "--property=KillMode=control-group",
        "--property=PrivateNetwork=yes",
        "--property=NoNewPrivileges=yes",
        "--property=CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_KILL",
        "--property=RestrictSUIDSGID=yes",
        "--property=TasksMax=32","--property=MemoryMax=536870912",
        "--property=RuntimeMaxSec=90",
        str(PYTHON),str(MANAGER),"--domain","probe",
        "--nonce",args.nonce,"--chat",args.chat,"--turn",args.turn,
        "--call",args.call,"--uid",str(uid),
        "--op",args.kind,"--binary",str(binary),
        "--binary-sha",args.binary_sha,
        "--file",str(resource),"--resource-sha",args.resource_sha,
    ]
    process=subprocess.run(command,cwd="/",timeout=105,check=False)
    if not receipt.is_file() or receipt.is_symlink():
        deny("root-owned kernel manager issued no independent media receipt")
    st=receipt.stat()
    if st.st_uid!=0 or st.st_nlink!=1 or st.st_mode&0o022 or st.st_size>1024:
        deny("original root-owned probe receipt is altered")
    with receipt.open("r",encoding="ascii") as f:
        proof=f.read(1025)
    prefix="\n".join([
        "LU_SEC06C_OWNED_PROBE_V1",
        "NONCE="+args.nonce,"CHAT="+args.chat,"TURN="+args.turn,
        "CALL="+args.call,"KIND="+args.kind,
        "BINARY_SHA256="+args.binary_sha,
        "RESOURCE_SHA256="+args.resource_sha,
        "UNIT="+unit,"ORIGINAL_UID="+str(uid),
        "KERNEL_EMPTY=1","EFFECT=",
    ])
    if proof==prefix+"COMPLETED\n" and process.returncode==0:
        return 0
    if proof==prefix+"INTERRUPTED\n" and process.returncode!=0:
        return 73
    deny("original probe OS process and root-owned kernel receipt disagree")

def main():
    ap=argparse.ArgumentParser(allow_abbrev=False)
    for arg in ("nonce","chat","turn","call","kind","binary","binary_sha","file","resource_sha"):
        ap.add_argument("--"+arg.replace("_","-"),required=True)
    try:return run(ap.parse_args())
    except Exception as exc:
        print("SEC06C_OS_PROBE_BROKER_BLOCKED: "+str(exc),file=sys.stderr)
        return 91
if __name__=="__main__":sys.exit(main())
