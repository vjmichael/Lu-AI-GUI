#!/usr/bin/python3
"""Lu SEC-06C first-party, *unprivileged* Git-read worker.

Only a root-owned systemd unit created by the fixed broker may execute this
program. It never accepts arbitrary commands, shell snippets, or model args.
The host's reviewed Git digest is verified again immediately before execution.
"""
import argparse
import hashlib
import os
import pathlib
import select
import stat
import subprocess
import sys
import tempfile
import time

READY = pathlib.Path("/run/lu-sec06c")
OPS = {
    "head": ["rev-parse", "--verify", "HEAD"],
    "status": ["status", "--porcelain=v1", "--untracked-files=no"],
    "branch": ["branch", "--show-current", "--no-color"],
    "count": ["rev-list", "--count", "HEAD"],
}
MAX_OUT = 65536

def deny(message):
    raise RuntimeError("SEC06C_GIT_WORKER_DENIED: " + message)

def token(value):
    return len(value) == 32 and all(c in "0123456789abcdef" for c in value)

def verify_source(path, expected):
    p = pathlib.Path(path)
    if not p.is_absolute() or p.name != "git" or p.is_symlink():
        deny("reviewed executable must be a canonical absolute Git binary")
    f = p.resolve(strict=True)
    if f != p or not f.is_file():
        deny("reviewed Git executable changed")
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
        deny("invalid reviewed digest")
    h = hashlib.sha256()
    with f.open("rb") as stream:
        for chunk in iter(lambda: stream.read(65536), b""):
            h.update(chunk)
    if h.hexdigest() != expected:
        deny("reviewed Git digest changed")
    return f

def kernel_unit(nonce):
    if os.geteuid() == 0:
        deny("worker cannot run as root")
    rows = pathlib.Path("/proc/self/cgroup").read_text().splitlines()
    if len(rows) != 1 or not rows[0].startswith("0::/"):
        deny("missing genuine unified cgroup-v2 membership")
    # ProtectControlGroups=strict creates a private cgroup namespace, where
    # /proc/self/cgroup often shows 0::/. The privileged manager checks the
    # original MainPID's full unit membership OUTSIDE this namespace before
    # issuing a root-owned one-use GO file; the worker verifies that file.
    relative = rows[0][3:]
    if relative != "/" and not relative.endswith(
            "/lu-sec06c-git-" + nonce + ".service"):
        deny("not running inside the protected systemd operation unit")
    group = (pathlib.Path("/sys/fs/cgroup") / relative.lstrip("/")).resolve(strict=True)
    if group.stat().st_uid == os.geteuid():
        deny("worker owns its own process-control boundary")
    procs = group / "cgroup.procs"
    if not procs.is_file() or os.access(procs, os.W_OK):
        deny("operation cgroup is writable to worker")
    if relative != "/":
        parent = group.parent / "cgroup.procs"
        if not parent.is_file() or os.access(parent, os.W_OK):
            deny("ancestor migration is writable to worker")
    # systemd ProtectControlGroups=strict makes this a read-only mount.
    mountinfo = pathlib.Path("/proc/self/mountinfo").read_text().splitlines()
    mounts = [r for r in mountinfo
              if " - cgroup2 " in r and r.split(" - ", 1)[0].split()[4] == "/sys/fs/cgroup"]
    if len(mounts) != 1 or "ro" not in mounts[0].split(" - ", 1)[0].split()[5].split(","):
        deny("ProtectControlGroups=strict was not actually enforced")

def authorized_go(nonce, timeout=20):
    root = READY
    if root.stat().st_uid != 0 or root.stat().st_mode & 0o022:
        deny("trusted GO parent not protected")
    path = root / (nonce + ".go")
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        # A dying original Lu host must not leave a gate-waiting worker
        # eligible to start Git on a stale, subsequently delivered GO.
        waiting, _, _ = select.select([sys.stdin.buffer], [], [], 0)
        if waiting and not sys.stdin.buffer.read(1):
            deny("original Lu host died before one-use admission")
        try:
            meta = path.lstat()
            if (not stat.S_ISREG(meta.st_mode) or meta.st_uid != 0
                    or meta.st_mode & 0o022 or meta.st_nlink != 1):
                deny("GO is not root-owned")
            with path.open("r", encoding="ascii") as stream:
                if stream.read() != "LU_SEC06C_GO_V1\n" + nonce + "\n":
                    deny("GO identity mismatch")
            return
        except FileNotFoundError:
            time.sleep(.025)
    deny("trusted manager never admitted first-worker launch")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nonce", required=True)
    ap.add_argument("--git", required=True)
    ap.add_argument("--sha", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--op", choices=sorted(OPS), required=True)
    args = ap.parse_args()
    if not token(args.nonce):
        deny("invalid operation identity")
    kernel_unit(args.nonce)
    git = verify_source(args.git, args.sha)
    repo = pathlib.Path(args.repo).resolve(strict=True)
    if not repo.is_dir() or not ((repo / ".git").is_dir() or (repo / ".git").is_file()):
        deny("not a reviewed Git repository")
    authorized_go(args.nonce)
    waiting, _, _ = select.select([sys.stdin.buffer], [], [], 0)
    if waiting and not sys.stdin.buffer.read(1):
        deny("original Lu host cancelled before reviewed Git spawn")
    argv = [str(git), "-c", "core.fsmonitor=false",
            "--no-optional-locks", "-C", str(repo), *OPS[args.op]]
    with tempfile.TemporaryFile() as output:
        child = subprocess.Popen(
            argv, stdin=subprocess.DEVNULL, stdout=output,
            stderr=subprocess.DEVNULL, env={
                "PATH": "/usr/bin:/bin", "LC_ALL": "C",
                "GIT_CONFIG_NOSYSTEM": "1",
            })
        try:
            deadline = time.monotonic() + 60
            while child.poll() is None:
                if time.monotonic() >= deadline:
                    child.kill()
                    child.wait()
                    deny("reviewed Git runtime limit")
                # The trusted Lu host owns the lifetime pipe. Its crash or
                # cancellation closes stdin, even if the reviewed Git forks.
                ready, _, _ = select.select([sys.stdin.buffer], [], [], .04)
                if ready and not sys.stdin.buffer.read(1):
                    child.kill()
                    child.wait()
                    deny("reviewed Lu host lifetime ended")
            if child.returncode != 0:
                deny("reviewed Git returned a failure")
            if output.tell() > MAX_OUT:
                deny("reviewed Git output exceeded bounded protocol")
            output.seek(0)
            sys.stdout.buffer.write(output.read(MAX_OUT + 1))
            sys.stdout.buffer.flush()
            return 0
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()

if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(f"SEC06C_GIT_WORKER_BLOCKED: {exc}", file=sys.stderr)
        sys.exit(93)
