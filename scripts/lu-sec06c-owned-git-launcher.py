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
    if os.geteuid() != 0 or "SUDO_UID" not in os.environ:
        deny("broker needs explicit OS approval, not a root worker process")
    caller = int(os.environ["SUDO_UID"])
    if caller <= 0 or caller == 0 or pwd.getpwuid(caller).pw_uid != caller:
        deny("caller is not a valid original nonroot user")
    if any(not ID.fullmatch(x) for x in [args.chat, args.turn, args.call]):
        deny("invalid original host authority identity")
    if not NONCE.fullmatch(args.nonce) or args.op not in OPS:
        deny("invalid one-use native operation")
    root_owned_file(pathlib.Path(__file__).resolve())
    root_owned_file(WORKER)
    root_owned_file(WORKER_PYTHON)
    root_owned_file(RUN)
    root_owned_file(CTL)
    git = pinned_git(args.git, args.sha)
    repo = pathlib.Path(args.repo).resolve(strict=True)
    if not pathlib.Path(args.repo).is_absolute() or not repo.is_dir():
        deny("untrusted reviewed repository")
    if not (repo.joinpath(".git").exists()):
        deny("reviewed repository not enrolled")
    root_owned_dir(READY)
    root_owned_dir(RECEIPTS)
    nonce = args.nonce
    unit = f"lu-sec06c-git-{nonce}.service"
    result_path = RECEIPTS / (nonce + ".receipt")
    go = READY / (nonce + ".go")
    if result_path.exists() or go.exists():
        deny("one-use operation nonce has already been admitted")
    worker_argv = [
        str(WORKER_PYTHON), str(WORKER),
        "--nonce", nonce, "--git", str(git),
        "--sha", args.sha, "--repo", str(repo), "--op", args.op,
    ]
    service = [
        str(RUN), "--system", "--quiet", "--pipe", "--wait",
        "--service-type=exec", "--no-ask-password", f"--unit={unit}",
        f"--property=User={caller}", "--property=Delegate=no",
        "--property=KillMode=control-group",
        "--property=ProtectControlGroups=strict",
        "--property=NoNewPrivileges=yes",
        "--property=CapabilityBoundingSet=",
        "--property=RestrictSUIDSGID=yes",
        f"--working-directory={repo}", *worker_argv,
    ]
    process = subprocess.Popen(service, stdin=None, stdout=None, stderr=None,
                               cwd="/")
    cgroup = None
    events = None
    started = False
    status = 99
    try:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                deny("system manager exited before secure worker admission")
            cgroup = kernel_group(unit)
            if cgroup is not None:
                break
            time.sleep(.04)
        if cgroup is None:
            deny("system manager never created a genuine root-owned unit")
        # Pin the genuine kernel events FD *before* the unit is stopped.
        events = (cgroup / "cgroup.events").open("r", encoding="ascii")
        if not (cgroup / "cgroup.kill").is_file():
            deny("systemd operation lacks real kernel cgroup.kill")
        # The host's one-use original lifetime pipe must still be live at
        # the exact authorization point; SIGKILL/EOF forbids first launch.
        waiting, _, _ = select.select([sys.stdin.buffer], [], [], 0)
        if waiting and not sys.stdin.buffer.read(1):
            deny("original host lifetime ended before systemd GO")
        create_root_file(go, "LU_SEC06C_GO_V1\n" + nonce + "\n")
        started = True
        try:
            status = process.wait(timeout=75)
        except subprocess.TimeoutExpired:
            deny("managed one-shot operation timed out")
    finally:
        # Only the exact random unit created here is stopped. Neither
        # a model-selected unit nor an arbitrary PID can be signalled.
        if started or cgroup is not None:
            subprocess.run([str(CTL), "stop", unit], check=False, timeout=15,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if process.poll() is None:
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        if go.exists():
            go.unlink()
    if events is None or not kernel_empty(events):
        deny("kernel did not attest recursive cgroup.events populated-zero")
    events.close()
    # This historical cleanup receipt is owned by root in a root-owned
    # directory, not by a worker/model or a mutable Lu profile.
    receipt = "\n".join([
        PROOF, "NONCE=" + nonce, "CHAT=" + args.chat,
        "TURN=" + args.turn, "CALL=" + args.call,
        "GIT_SHA256=" + args.sha, "UNIT=" + unit,
        "ORIGINAL_UID=" + str(caller), "KERNEL_EMPTY=1",
        "EFFECT=" + ("COMPLETED" if status == 0 else "INTERRUPTED"), "",
    ])
    create_root_file(result_path, receipt)
    return 0 if status == 0 else 73

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
