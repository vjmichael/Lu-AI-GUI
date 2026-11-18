#!/usr/bin/python3
"""Lu SEC-06C fixed, privileged systemd operation launcher.

Install ROOT-OWNED at /usr/libexec/lu/lu-sec06c-launch-git with a narrowly
reviewed policy that permits invoking THIS FILE only. No sudo systemd-run
authorization is granted to Lu. The only transient service executable is the
root-owned FIRST-PARTY Git-read worker, running as the invoking NONROOT UID.
All Git operations remain read-only, host-reviewed and digest-pinned.
"""
import argparse
import hashlib
import os
import pathlib
import pwd
import select
import re
import stat
import subprocess
import sys
import time

WORKER = pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-git-worker")
WORKER_PYTHON = pathlib.Path("/usr/bin/python3").resolve(strict=True)
RUN = pathlib.Path("/usr/bin/systemd-run")
CTL = pathlib.Path("/usr/bin/systemctl")
READY = pathlib.Path("/run/lu-sec06c")
RECEIPTS = pathlib.Path("/var/lib/lu-sec06c/receipts")
MANAGER = pathlib.Path("/usr/libexec/lu/lu-sec06c-owned-kernel-manager")
PROOF = "LU_SEC06C_OWNED_GIT_V1"
OPS = {"head", "status", "branch", "count"}
ID = re.compile(r"^[a-zA-Z0-9_-]{1,128}$")
NONCE = re.compile(r"^[a-f0-9]{32}$")
DIGEST = re.compile(r"^[a-f0-9]{64}$")

def deny(why):
    raise RuntimeError("SEC06C_OS_BROKER_DENIED: " + why)

def root_owned_file(path):
    if not path.is_absolute() or path.is_symlink():
        deny("untrusted installed service binary")
    meta = path.stat()
    if not stat.S_ISREG(meta.st_mode) or meta.st_uid != 0 or meta.st_mode & 0o022:
        deny("service executable must be nonwritable and root owned")
    for parent in [path.parent, path.parent.parent]:
        st = parent.stat()
        if st.st_uid != 0 or st.st_mode & 0o022:
            deny("service executable directory is writable")
    return path

def root_owned_dir(path):
    path.mkdir(parents=True, mode=0o755, exist_ok=True)
    if path.is_symlink():
        deny("root-owned OS receipt folder is linked")
    st = path.stat()
    if st.st_uid != 0 or not stat.S_ISDIR(st.st_mode) or st.st_mode & 0o022:
        deny("root-owned OS receipt folder is untrusted")

def pinned_git(path, digest):
    p = pathlib.Path(path)
    if not p.is_absolute() or p.name != "git" or p.is_symlink():
        deny("only a canonical absolute Git executable is allowed")
    real = p.resolve(strict=True)
    if real != p or not real.is_file() or real.stat().st_uid != 0:
        deny("Git executable is not a root-owned reviewed file")
    if real.stat().st_mode & 0o022:
        deny("reviewed Git executable is writable by untrusted users")
    if not DIGEST.fullmatch(digest):
        deny("invalid reviewed executable digest")
    check = hashlib.sha256()
    with real.open("rb") as f:
        for data in iter(lambda: f.read(65536), b""):
            check.update(data)
    if check.hexdigest() != digest:
        deny("reviewed Git digest changed")
    return real

def kernel_group(unit):
    show = subprocess.run(
        [str(CTL), "show", unit, "-p", "MainPID", "-p", "ControlGroup",
         "-p", "ActiveState", "--no-pager"], text=True,
        capture_output=True, timeout=4)
    if show.returncode:
        return None
    props = dict(row.split("=", 1) for row in show.stdout.splitlines() if "=" in row)
    group = props.get("ControlGroup", "")
    pid = props.get("MainPID", "")
    if not group.startswith("/") or not group.endswith("/" + unit) or not pid.isdigit():
        return None
    if int(pid) <= 1:
        return None
    p = pathlib.Path("/sys/fs/cgroup") / group.lstrip("/")
    if not p.is_dir():
        return None
    if p.stat().st_uid != 0:
        deny("OS operation unit is not system-owned")
    member = pathlib.Path(f"/proc/{pid}/cgroup").read_text(encoding="ascii").strip()
    if member != "0::" + group:
        deny("unit MainPID kernel membership does not match")
    return p

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
        lines = stream.read().splitlines()
        values = [row for row in lines if row.startswith("populated ")]
        return values == ["populated 0"]
    except OSError:
        return False

def create_root_file(path, payload):
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
    fd = os.open(path, flags, 0o644)
    with os.fdopen(fd, "w", encoding="ascii") as f:
        f.write(payload)
        f.flush()
        os.fsync(f.fileno())
    directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)

def run(args):
    # The only privileged surface is this installed, root-owned *finite* Git
    # launcher; it never accepts an arbitrary manager/worker program, shell
    # command or model-selected systemd unit.
    if os.geteuid()!=0 or "SUDO_UID" not in os.environ:
        deny("fixed Git broker requires original sudo admission")
    caller=int(os.environ["SUDO_UID"])
    if caller<=0 or pwd.getpwuid(caller).pw_uid!=caller:
        deny("original Lu caller must remain nonroot")
    if any(not ID.fullmatch(x) for x in [args.chat,args.turn,args.call]):
        deny("original human task identity invalid")
    if not NONCE.fullmatch(args.nonce) or args.op not in OPS:
        deny("original one-use Git operation invalid")
    for file in (pathlib.Path(__file__).resolve(),WORKER,WORKER_PYTHON,
                 MANAGER,RUN,CTL):
        root_owned_file(file)
    git=pinned_git(args.git,args.sha)
    repo=pathlib.Path(args.repo).resolve(strict=True)
    if not pathlib.Path(args.repo).is_absolute() or not repo.is_dir():
        deny("reviewed Git repository not canonical")
    if not (repo/".git").is_dir():
        deny("reviewed Git repository not enrolled")
    root_owned_dir(READY)
    root_owned_dir(RECEIPTS)
    receipt=RECEIPTS/(args.nonce+".receipt")
    go=READY/(args.nonce+".go")
    if receipt.exists() or go.exists():
        deny("original Git nonce already consumed")
    unit=f"lu-sec06c-git-{args.nonce}.service"
    # Root-owned SYSTEMD outer cgroup. The separately installed first-party
    # root supervisor stays in this unit, creates one ROOT-owned child leaf
    # under Delegate=yes and drops the reviewed worker to the original UID
    # inside that leaf. The unprivileged worker can neither move to the root
    # parent nor make cgroupfs writable in its private mount namespace.
    argv=[
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
        "--property=RuntimeMaxSec=90",
        str(WORKER_PYTHON),str(MANAGER),"--domain","git",
        "--nonce",args.nonce,"--chat",args.chat,"--turn",args.turn,
        "--call",args.call,"--uid",str(caller),
        "--git",str(git),"--sha",args.sha,
        "--repo",str(repo),"--op",args.op,
    ]
    process=subprocess.run(argv,cwd="/",timeout=105,check=False)
    # A systemd-run client success is *never* independent kernel custody.
    # Read exactly the root supervisor's own one-use receipt and original
    # kernel leaf fields; immutable Lu journal verifies it separately too.
    if not receipt.is_file() or receipt.is_symlink():
        deny("root-owned inner kernel supervisor issued no original receipt")
    st=receipt.stat()
    if st.st_uid!=0 or st.st_nlink!=1 or st.st_mode&0o022 or st.st_size>1024:
        deny("kernel supervisor receipt is untrusted")
    with receipt.open("r",encoding="ascii") as f:
        proof=f.read(1025)
    expected="\n".join([
        PROOF,"NONCE="+args.nonce,"CHAT="+args.chat,"TURN="+args.turn,
        "CALL="+args.call,"GIT_SHA256="+args.sha,
        "UNIT="+unit,"ORIGINAL_UID="+str(caller),
        "KERNEL_EMPTY=1","EFFECT=",
    ])
    if proof==expected+"COMPLETED\n" and process.returncode==0:
        return 0
    if proof==expected+"INTERRUPTED\n" and process.returncode!=0:
        return 73
    deny("original fixed Git OS process and root kernel receipt disagree")

def main():
    ap = argparse.ArgumentParser(allow_abbrev=False)
    for name in ["nonce", "chat", "turn", "call", "git", "sha", "repo"]:
        ap.add_argument("--" + name, required=True)
    ap.add_argument("--op", choices=sorted(OPS), required=True)
    args = ap.parse_args()
    try:
        return run(args)
    except Exception as exc:
        print(f"SEC06C_OS_BROKER_BLOCKED: {exc}", file=sys.stderr)
        return 91

if __name__ == "__main__":
    sys.exit(main())
